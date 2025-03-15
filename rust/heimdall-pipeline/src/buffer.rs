use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use crossbeam::epoch::{self, Atomic, Owned, Shared};
use crossbeam_utils::CachePadded;
use log::{debug, error, info, warn};
use crate::{OverflowStrategy, PipelineError};
use crate::timestamp::Timestamp;

/// Structure d'une image dans le buffer
#[repr(C, align(64))] // Alignement sur 64 octets pour éviter le false sharing
pub struct ImageSlot {
    /// Données de l'image
    pub data: Box<[u8]>,
    
    /// Taille effective des données
    pub size: usize,
    
    /// Largeur de l'image
    pub width: u32,
    
    /// Hauteur de l'image
    pub height: u32,
    
    /// Format de pixel
    pub format: u32,
    
    /// Horodatage précis de l'acquisition
    pub timestamp: Timestamp,
    
    /// Numéro de séquence de l'image
    pub sequence: u64,
    
    /// Métadonnées supplémentaires (jusqu'à 8 valeurs u64)
    pub metadata: [u64; 8],
    
    /// Drapeau indiquant si le slot est valide
    pub valid: AtomicBool,
}

impl ImageSlot {
    /// Crée un nouveau slot d'image avec la capacité spécifiée
    pub fn new(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity].into_boxed_slice(),
            size: 0,
            width: 0,
            height: 0,
            format: 0,
            timestamp: Timestamp::now(),
            sequence: 0,
            metadata: [0; 8],
            valid: AtomicBool::new(false),
        }
    }
    
    /// Réinitialise le slot
    pub fn reset(&mut self) {
        self.size = 0;
        self.width = 0;
        self.height = 0;
        self.format = 0;
        self.sequence = 0;
        self.metadata = [0; 8];
        self.valid.store(false, Ordering::Release);
    }
}

/// Buffer circulaire lock-free pour les images
pub struct LockFreeRingBuffer {
    /// Slots d'images
    slots: Box<[CachePadded<Atomic<ImageSlot>>]>,
    
    /// Capacité du buffer
    capacity: usize,
    
    /// Index de la tête (prochain emplacement à écrire)
    head: CachePadded<AtomicUsize>,
    
    /// Index de la queue (prochain emplacement à lire)
    tail: CachePadded<AtomicUsize>,
    
    /// Nombre d'éléments dans le buffer
    size: CachePadded<AtomicUsize>,
    
    /// Compteur d'images produites
    produced: CachePadded<AtomicU64>,
    
    /// Compteur d'images consommées
    consumed: CachePadded<AtomicU64>,
    
    /// Compteur d'images perdues
    dropped: CachePadded<AtomicU64>,
    
    /// Stratégie de gestion des débordements
    overflow_strategy: OverflowStrategy,
    
    /// Taille maximale d'une image
    max_image_size: usize,
}

impl LockFreeRingBuffer {
    /// Crée un nouveau buffer circulaire lock-free
    pub fn new(capacity: usize, max_image_size: usize, strategy: OverflowStrategy) -> Self {
        let mut slots = Vec::with_capacity(capacity);
        
        // Initialiser les slots
        for _ in 0..capacity {
            slots.push(CachePadded::new(Atomic::new(ImageSlot::new(max_image_size))));
        }
        
        Self {
            slots: slots.into_boxed_slice(),
            capacity,
            head: CachePadded::new(AtomicUsize::new(0)),
            tail: CachePadded::new(AtomicUsize::new(0)),
            size: CachePadded::new(AtomicUsize::new(0)),
            produced: CachePadded::new(AtomicU64::new(0)),
            consumed: CachePadded::new(AtomicU64::new(0)),
            dropped: CachePadded::new(AtomicU64::new(0)),
            overflow_strategy: strategy,
            max_image_size,
        }
    }
    
    /// Obtient la capacité du buffer
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    
    /// Obtient le nombre d'éléments dans le buffer
    pub fn len(&self) -> usize {
        self.size.load(Ordering::Relaxed)
    }
    
    /// Vérifie si le buffer est vide
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    
    /// Vérifie si le buffer est plein
    pub fn is_full(&self) -> bool {
        self.len() >= self.capacity
    }
    
    /// Obtient le nombre d'images produites
    pub fn produced_count(&self) -> u64 {
        self.produced.load(Ordering::Relaxed)
    }
    
    /// Obtient le nombre d'images consommées
    pub fn consumed_count(&self) -> u64 {
        self.consumed.load(Ordering::Relaxed)
    }
    
    /// Obtient le nombre d'images perdues
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
    
    /// Réserve un slot pour écriture
    pub fn reserve_write_slot(&self) -> Result<(usize, &mut ImageSlot), PipelineError> {
        // Vérifier si le buffer est plein
        if self.is_full() {
            match self.overflow_strategy {
                OverflowStrategy::Block => {
                    // Attendre qu'un slot se libère (non recommandé en temps réel)
                    return Err(PipelineError::BufferError("Buffer plein (stratégie Block)".to_string()));
                },
                OverflowStrategy::DropNewest => {
                    // Rejeter la nouvelle image
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                    return Err(PipelineError::BufferError("Buffer plein (stratégie DropNewest)".to_string()));
                },
                OverflowStrategy::DropOldest => {
                    // Supprimer l'image la plus ancienne
                    self.force_advance_tail();
                },
                OverflowStrategy::Resize => {
                    // Non implémenté (causerait des allocations)
                    return Err(PipelineError::BufferError("Redimensionnement du buffer non implémenté".to_string()));
                },
            }
        }
        
        // Obtenir l'index de la tête
        let head = self.head.load(Ordering::Relaxed);
        let slot_index = head % self.capacity;
        
        // Utiliser epoch-based reclamation pour la sécurité mémoire
        let guard = &epoch::pin();
        
        // Créer un nouveau slot
        let mut new_slot = ImageSlot::new(self.max_image_size);
        new_slot.reset();
        
        // Remplacer l'ancien slot par le nouveau
        let old_slot = self.slots[slot_index].swap(Owned::new(new_slot), Ordering::AcqRel, guard);
        
        // Avancer la tête
        self.head.fetch_add(1, Ordering::Release);
        self.size.fetch_add(1, Ordering::Release);
        
        // Incrémenter le compteur d'images produites
        self.produced.fetch_add(1, Ordering::Relaxed);
        
        // Obtenir une référence mutable au slot
        let slot_ref = unsafe {
            // Safety: Nous venons de créer ce slot et nous avons l'accès exclusif
            &mut *self.slots[slot_index].load(Ordering::Acquire, guard).as_raw()
        };
        
        Ok((slot_index, slot_ref))
    }
    
    /// Force l'avancement de la queue (supprime l'élément le plus ancien)
    fn force_advance_tail(&self) {
        // Obtenir l'index de la queue
        let tail = self.tail.load(Ordering::Relaxed);
        let slot_index = tail % self.capacity;
        
        // Utiliser epoch-based reclamation pour la sécurité mémoire
        let guard = &epoch::pin();
        
        // Marquer le slot comme invalide
        let slot = unsafe {
            // Safety: Nous accédons au slot pour le marquer comme invalide
            &*self.slots[slot_index].load(Ordering::Acquire, guard).as_raw()
        };
        slot.valid.store(false, Ordering::Release);
        
        // Avancer la queue
        self.tail.fetch_add(1, Ordering::Release);
        self.size.fetch_sub(1, Ordering::Release);
        
        // Incrémenter le compteur d'images perdues
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Finalise l'écriture d'un slot
    pub fn commit_write(&self, slot_index: usize, sequence: u64) {
        // Utiliser epoch-based reclamation pour la sécurité mémoire
        let guard = &epoch::pin();
        
        // Obtenir une référence au slot
        let slot = unsafe {
            // Safety: Nous avons l'accès exclusif à ce slot
            &*self.slots[slot_index].load(Ordering::Acquire, guard).as_raw()
        };
        
        // Mettre à jour le numéro de séquence
        let slot_mut = unsafe {
            // Safety: Nous avons l'accès exclusif à ce slot
            &mut *(slot as *const ImageSlot as *mut ImageSlot)
        };
        slot_mut.sequence = sequence;
        
        // Marquer le slot comme valide
        slot.valid.store(true, Ordering::Release);
    }
    
    /// Lit le prochain slot disponible
    pub fn read_slot(&self) -> Result<(usize, &ImageSlot), PipelineError> {
        // Vérifier si le buffer est vide
        if self.is_empty() {
            return Err(PipelineError::BufferError("Buffer vide".to_string()));
        }
        
        // Obtenir l'index de la queue
        let tail = self.tail.load(Ordering::Relaxed);
        let slot_index = tail % self.capacity;
        
        // Utiliser epoch-based reclamation pour la sécurité mémoire
        let guard = &epoch::pin();
        
        // Obtenir une référence au slot
        let slot = unsafe {
            // Safety: Nous accédons au slot en lecture seule
            &*self.slots[slot_index].load(Ordering::Acquire, guard).as_raw()
        };
        
        // Vérifier si le slot est valide
        if !slot.valid.load(Ordering::Acquire) {
            return Err(PipelineError::BufferError("Slot invalide".to_string()));
        }
        
        Ok((slot_index, slot))
    }
    
    /// Finalise la lecture d'un slot
    pub fn commit_read(&self, slot_index: usize) {
        // Utiliser epoch-based reclamation pour la sécurité mémoire
        let guard = &epoch::pin();
        
        // Obtenir une référence au slot
        let slot = unsafe {
            // Safety: Nous accédons au slot pour le marquer comme invalide
            &*self.slots[slot_index].load(Ordering::Acquire, guard).as_raw()
        };
        
        // Marquer le slot comme invalide
        slot.valid.store(false, Ordering::Release);
        
        // Avancer la queue
        self.tail.fetch_add(1, Ordering::Release);
        self.size.fetch_sub(1, Ordering::Release);
        
        // Incrémenter le compteur d'images consommées
        self.consumed.fetch_add(1, Ordering::Relaxed);
    }
    
    /// Réinitialise le buffer
    pub fn reset(&self) {
        // Réinitialiser les compteurs
        self.head.store(0, Ordering::Relaxed);
        self.tail.store(0, Ordering::Relaxed);
        self.size.store(0, Ordering::Relaxed);
        
        // Utiliser epoch-based reclamation pour la sécurité mémoire
        let guard = &epoch::pin();
        
        // Réinitialiser tous les slots
        for i in 0..self.capacity {
            let slot = unsafe {
                // Safety: Nous réinitialisons tous les slots
                &mut *self.slots[i].load(Ordering::Acquire, guard).as_raw()
            };
            slot.valid.store(false, Ordering::Release);
        }
    }
}

/// Implémentation du Drop pour libérer les ressources
impl Drop for LockFreeRingBuffer {
    fn drop(&mut self) {
        // Rien de spécial à faire ici, les slots seront libérés automatiquement
    }
}

/// Tests unitaires
#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    
    #[test]
    fn test_buffer_basic_operations() {
        let buffer = LockFreeRingBuffer::new(4, 1024, OverflowStrategy::DropOldest);
        
        // Vérifier l'état initial
        assert_eq!(buffer.capacity(), 4);
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
        
        // Écrire un élément
        let (slot_index, slot) = buffer.reserve_write_slot().unwrap();
        slot.size = 100;
        slot.width = 10;
        slot.height = 10;
        buffer.commit_write(slot_index, 1);
        
        // Vérifier l'état après écriture
        assert_eq!(buffer.len(), 1);
        assert!(!buffer.is_empty());
        assert!(!buffer.is_full());
        
        // Lire l'élément
        let (read_index, read_slot) = buffer.read_slot().unwrap();
        assert_eq!(read_slot.size, 100);
        assert_eq!(read_slot.width, 10);
        assert_eq!(read_slot.height, 10);
        assert_eq!(read_slot.sequence, 1);
        buffer.commit_read(read_index);
        
        // Vérifier l'état après lecture
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
        assert!(!buffer.is_full());
    }
    
    #[test]
    fn test_buffer_overflow_drop_oldest() {
        let buffer = LockFreeRingBuffer::new(2, 1024, OverflowStrategy::DropOldest);
        
        // Remplir le buffer
        let (slot_index1, slot1) = buffer.reserve_write_slot().unwrap();
        slot1.sequence = 1;
        buffer.commit_write(slot_index1, 1);
        
        let (slot_index2, slot2) = buffer.reserve_write_slot().unwrap();
        slot2.sequence = 2;
        buffer.commit_write(slot_index2, 2);
        
        // Vérifier que le buffer est plein
        assert_eq!(buffer.len(), 2);
        assert!(buffer.is_full());
        
        // Ajouter un élément supplémentaire (devrait supprimer le plus ancien)
        let (slot_index3, slot3) = buffer.reserve_write_slot().unwrap();
        slot3.sequence = 3;
        buffer.commit_write(slot_index3, 3);
        
        // Vérifier que le buffer est toujours plein
        assert_eq!(buffer.len(), 2);
        assert!(buffer.is_full());
        
        // Lire le premier élément (devrait être le 2ème)
        let (read_index, read_slot) = buffer.read_slot().unwrap();
        assert_eq!(read_slot.sequence, 2);
        buffer.commit_read(read_index);
        
        // Lire le deuxième élément (devrait être le 3ème)
        let (read_index, read_slot) = buffer.read_slot().unwrap();
        assert_eq!(read_slot.sequence, 3);
        buffer.commit_read(read_index);
        
        // Vérifier que le buffer est vide
        assert_eq!(buffer.len(), 0);
        assert!(buffer.is_empty());
    }
    
    #[test]
    fn test_buffer_concurrent_operations() {
        let buffer = Arc::new(LockFreeRingBuffer::new(1000, 1024, OverflowStrategy::DropOldest));
        
        // Créer des threads producteurs
        let mut producer_handles = vec![];
        let producer_count = 4;
        let items_per_producer = 1000;
        
        for p in 0..producer_count {
            let buffer_clone = buffer.clone();
            let handle = thread::spawn(move || {
                for i in 0..items_per_producer {
                    let sequence = p * items_per_producer + i;
                    match buffer_clone.reserve_write_slot() {
                        Ok((slot_index, slot)) => {
                            slot.sequence = sequence as u64;
                            buffer_clone.commit_write(slot_index, sequence as u64);
                        },
                        Err(_) => {
                            // Ignorer les erreurs (buffer plein)
                        }
                    }
                }
            });
            producer_handles.push(handle);
        }
        
        // Créer des threads consommateurs
        let mut consumer_handles = vec![];
        let consumer_count = 2;
        let total_items = producer_count * items_per_producer;
        let items_per_consumer = total_items / consumer_count;
        
        let consumed = Arc::new(AtomicUsize::new(0));
        
        for _ in 0..consumer_count {
            let buffer_clone = buffer.clone();
            let consumed_clone = consumed.clone();
            let handle = thread::spawn(move || {
                let mut local_consumed = 0;
                
                while local_consumed < items_per_consumer {
                    match buffer_clone.read_slot() {
                        Ok((slot_index, _)) => {
                            buffer_clone.commit_read(slot_index);
                            local_consumed += 1;
                        },
                        Err(_) => {
                            // Attendre un peu si le buffer est vide
                            thread::yield_now();
                        }
                    }
                }
                
                consumed_clone.fetch_add(local_consumed, Ordering::Relaxed);
            });
            consumer_handles.push(handle);
        }
        
        // Attendre que tous les producteurs terminent
        for handle in producer_handles {
            handle.join().unwrap();
        }
        
        // Attendre que tous les consommateurs terminent
        for handle in consumer_handles {
            handle.join().unwrap();
        }
        
        // Vérifier que tous les éléments ont été consommés ou perdus
        let total_consumed = consumed.load(Ordering::Relaxed);
        let total_dropped = buffer.dropped_count() as usize;
        
        assert_eq!(total_consumed + total_dropped, total_items);
    }
}