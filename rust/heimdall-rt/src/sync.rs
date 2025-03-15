use crate::RtError;
use log::{debug, error, info, warn};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};
use parking_lot::{RwLock as PLRwLock, Mutex as PLMutex};
use crossbeam::queue::ArrayQueue;
use crossbeam::channel::{self, Sender, Receiver, TrySendError, TryRecvError};

/// File d'attente temps réel sans allocation
pub struct RtQueue<T> {
    /// File d'attente sous-jacente
    queue: Arc<ArrayQueue<T>>,
    
    /// Capacité de la file d'attente
    capacity: usize,
}

impl<T> RtQueue<T> {
    /// Crée une nouvelle file d'attente avec la capacité spécifiée
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
            capacity,
        }
    }
    
    /// Ajoute un élément à la file d'attente
    pub fn push(&self, item: T) -> Result<(), T> {
        self.queue.push(item)
    }
    
    /// Récupère un élément de la file d'attente
    pub fn pop(&self) -> Option<T> {
        self.queue.pop()
    }
    
    /// Vérifie si la file d'attente est vide
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
    
    /// Vérifie si la file d'attente est pleine
    pub fn is_full(&self) -> bool {
        self.queue.len() == self.capacity
    }
    
    /// Obtient le nombre d'éléments dans la file d'attente
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    
    /// Obtient la capacité de la file d'attente
    pub fn capacity(&self) -> usize {
        self.capacity
    }
}

/// Canal de communication temps réel
pub struct RtChannel<T> {
    /// Émetteur
    sender: Sender<T>,
    
    /// Récepteur
    receiver: Receiver<T>,
    
    /// Capacité du canal
    capacity: usize,
}

impl<T> RtChannel<T> {
    /// Crée un nouveau canal avec la capacité spécifiée
    pub fn new(capacity: usize) -> Self {
        let (sender, receiver) = channel::bounded(capacity);
        
        Self {
            sender,
            receiver,
            capacity,
        }
    }
    
    /// Envoie un élément dans le canal (non bloquant)
    pub fn try_send(&self, item: T) -> Result<(), TrySendError<T>> {
        self.sender.try_send(item)
    }
    
    /// Reçoit un élément du canal (non bloquant)
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.receiver.try_recv()
    }
    
    /// Envoie un élément dans le canal (bloquant)
    pub fn send(&self, item: T) -> Result<(), channel::SendError<T>> {
        self.sender.send(item)
    }
    
    /// Reçoit un élément du canal (bloquant)
    pub fn recv(&self) -> Result<T, channel::RecvError> {
        self.receiver.recv()
    }
    
    /// Envoie un élément dans le canal avec un timeout
    pub fn send_timeout(&self, item: T, timeout: Duration) -> Result<(), channel::SendTimeoutError<T>> {
        self.sender.send_timeout(item, timeout)
    }
    
    /// Reçoit un élément du canal avec un timeout
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, channel::RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
    
    /// Obtient la capacité du canal
    pub fn capacity(&self) -> usize {
        self.capacity
    }
    
    /// Obtient le nombre d'éléments dans le canal
    pub fn len(&self) -> usize {
        self.sender.len()
    }
    
    /// Vérifie si le canal est vide
    pub fn is_empty(&self) -> bool {
        self.sender.len() == 0
    }
    
    /// Vérifie si le canal est plein
    pub fn is_full(&self) -> bool {
        self.sender.len() == self.capacity
    }
}

/// Verrou en lecture/écriture optimisé pour les performances temps réel
pub struct RtRwLock<T> {
    /// Verrou sous-jacent
    lock: PLRwLock<T>,
}

impl<T> RtRwLock<T> {
    /// Crée un nouveau verrou avec la valeur spécifiée
    pub fn new(value: T) -> Self {
        Self {
            lock: PLRwLock::new(value),
        }
    }
    
    /// Acquiert le verrou en lecture
    pub fn read(&self) -> parking_lot::RwLockReadGuard<'_, T> {
        self.lock.read()
    }
    
    /// Acquiert le verrou en écriture
    pub fn write(&self) -> parking_lot::RwLockWriteGuard<'_, T> {
        self.lock.write()
    }
    
    /// Tente d'acquérir le verrou en lecture
    pub fn try_read(&self) -> Option<parking_lot::RwLockReadGuard<'_, T>> {
        self.lock.try_read()
    }
    
    /// Tente d'acquérir le verrou en écriture
    pub fn try_write(&self) -> Option<parking_lot::RwLockWriteGuard<'_, T>> {
        self.lock.try_write()
    }
}

/// Mutex optimisé pour les performances temps réel
pub struct RtMutex<T> {
    /// Mutex sous-jacent
    mutex: PLMutex<T>,
}

impl<T> RtMutex<T> {
    /// Crée un nouveau mutex avec la valeur spécifiée
    pub fn new(value: T) -> Self {
        Self {
            mutex: PLMutex::new(value),
        }
    }
    
    /// Acquiert le mutex
    pub fn lock(&self) -> parking_lot::MutexGuard<'_, T> {
        self.mutex.lock()
    }
    
    /// Tente d'acquérir le mutex
    pub fn try_lock(&self) -> Option<parking_lot::MutexGuard<'_, T>> {
        self.mutex.try_lock()
    }
}

/// Barrière de synchronisation
pub struct RtBarrier {
    /// Nombre de threads à attendre
    count: usize,
    
    /// Nombre de threads actuellement en attente
    waiting: Arc<Mutex<usize>>,
    
    /// Génération actuelle
    generation: Arc<Mutex<usize>>,
}

impl RtBarrier {
    /// Crée une nouvelle barrière pour le nombre spécifié de threads
    pub fn new(count: usize) -> Self {
        Self {
            count,
            waiting: Arc::new(Mutex::new(0)),
            generation: Arc::new(Mutex::new(0)),
        }
    }
    
    /// Attend que tous les threads atteignent la barrière
    pub fn wait(&self) -> Result<(), RtError> {
        let mut waiting = self.waiting.lock().map_err(|e| {
            RtError::SyncError(format!("Erreur lors de l'accès au compteur d'attente: {}", e))
        })?;
        
        let mut generation = self.generation.lock().map_err(|e| {
            RtError::SyncError(format!("Erreur lors de l'accès à la génération: {}", e))
        })?;
        
        let current_gen = *generation;
        
        *waiting += 1;
        
        if *waiting == self.count {
            // Tous les threads sont arrivés
            *waiting = 0;
            *generation += 1;
            
            Ok(())
        } else {
            // Attendre que tous les threads arrivent
            drop(waiting);
            drop(generation);
            
            let start = Instant::now();
            let timeout = Duration::from_secs(10);
            
            loop {
                if start.elapsed() > timeout {
                    return Err(RtError::TimeoutError("Timeout lors de l'attente à la barrière".to_string()));
                }
                
                let gen = self.generation.lock().map_err(|e| {
                    RtError::SyncError(format!("Erreur lors de l'accès à la génération: {}", e))
                })?;
                
                if *gen != current_gen {
                    // La génération a changé, tous les threads sont arrivés
                    break;
                }
                
                // Attendre un peu pour éviter de surcharger le CPU
                std::thread::yield_now();
            }
            
            Ok(())
        }
    }
}