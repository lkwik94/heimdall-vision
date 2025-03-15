use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use heimdall_pipeline::{
    PipelineConfig, OverflowStrategy, PipelineState,
    buffer::LockFreeRingBuffer,
    timestamp::Timestamp,
};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

fn bench_buffer_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("buffer_operations");
    
    for buffer_size in [16, 32, 64, 128, 256].iter() {
        group.bench_with_input(BenchmarkId::new("write_read", buffer_size), buffer_size, |b, &size| {
            let buffer = LockFreeRingBuffer::new(size, 1024, OverflowStrategy::DropOldest);
            
            b.iter(|| {
                // Réserver un slot
                let (slot_index, slot) = buffer.reserve_write_slot().unwrap();
                
                // Écrire des données
                slot.size = 100;
                slot.width = 10;
                slot.height = 10;
                slot.timestamp = Timestamp::now();
                
                // Finaliser l'écriture
                buffer.commit_write(slot_index, 1);
                
                // Lire le slot
                let (read_index, _) = buffer.read_slot().unwrap();
                
                // Finaliser la lecture
                buffer.commit_read(read_index);
            });
        });
        
        group.bench_with_input(BenchmarkId::new("concurrent_operations", buffer_size), buffer_size, |b, &size| {
            b.iter(|| {
                let buffer = Arc::new(LockFreeRingBuffer::new(size, 1024, OverflowStrategy::DropOldest));
                
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
                
                for _ in 0..consumer_count {
                    let buffer_clone = buffer.clone();
                    let handle = thread::spawn(move || {
                        let mut consumed = 0;
                        
                        while consumed < (producer_count * items_per_producer) / consumer_count {
                            match buffer_clone.read_slot() {
                                Ok((slot_index, _)) => {
                                    buffer_clone.commit_read(slot_index);
                                    consumed += 1;
                                },
                                Err(_) => {
                                    // Attendre un peu si le buffer est vide
                                    thread::yield_now();
                                }
                            }
                        }
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
            });
        });
    }
    
    group.finish();
}

fn bench_timestamp_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("timestamp_operations");
    
    group.bench_function("timestamp_creation", |b| {
        b.iter(|| {
            black_box(Timestamp::now());
        });
    });
    
    group.bench_function("timestamp_comparison", |b| {
        let ts1 = Timestamp::now();
        thread::sleep(Duration::from_millis(1));
        let ts2 = Timestamp::now();
        
        b.iter(|| {
            black_box(ts1.diff_nanos(&ts2));
            black_box(ts1 < ts2);
        });
    });
    
    group.bench_function("timestamp_arithmetic", |b| {
        let ts = Timestamp::now();
        let duration = Duration::from_millis(100);
        
        b.iter(|| {
            black_box(ts.add_duration(duration));
            black_box(ts.sub_duration(duration));
        });
    });
    
    group.finish();
}

criterion_group!(benches, bench_buffer_operations, bench_timestamp_operations);
criterion_main!(benches);