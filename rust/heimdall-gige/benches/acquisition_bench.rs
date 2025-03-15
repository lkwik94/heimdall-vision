//! Benchmark des performances d'acquisition
//!
//! Ce benchmark mesure les performances d'acquisition d'images
//! avec le module heimdall-gige.

use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use heimdall_gige::{GigESystem, SyncMode};
use tokio::runtime::Runtime;

fn bench_acquisition(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // Créer un groupe de benchmark
    let mut group = c.benchmark_group("acquisition");
    
    // Configurer le système GigE
    let mut gige = rt.block_on(async {
        let mut gige = GigESystem::new().unwrap();
        gige.discover_cameras().await.unwrap();
        gige.configure_cameras(SyncMode::Freerun).await.unwrap();
        gige.start_acquisition().await.unwrap();
        gige
    });
    
    // Benchmark d'acquisition d'une seule image
    group.bench_function(BenchmarkId::new("single_frame", ""), |b| {
        b.iter(|| {
            rt.block_on(async {
                let frames = black_box(gige.acquire_frames().await.unwrap());
                black_box(frames);
            });
        });
    });
    
    // Benchmark d'acquisition de 10 images consécutives
    group.bench_function(BenchmarkId::new("10_frames", ""), |b| {
        b.iter(|| {
            rt.block_on(async {
                for _ in 0..10 {
                    let frames = black_box(gige.acquire_frames().await.unwrap());
                    black_box(frames);
                }
            });
        });
    });
    
    // Arrêter l'acquisition
    rt.block_on(async {
        gige.stop_acquisition().await.unwrap();
    });
    
    group.finish();
}

fn bench_sync_modes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    
    // Créer un groupe de benchmark
    let mut group = c.benchmark_group("sync_modes");
    
    // Tester différents modes de synchronisation
    for sync_mode in [SyncMode::Freerun, SyncMode::Software, SyncMode::Hardware] {
        group.bench_with_input(BenchmarkId::from_parameter(format!("{:?}", sync_mode)), &sync_mode, |b, &sync_mode| {
            b.iter(|| {
                rt.block_on(async {
                    // Configurer le système GigE
                    let mut gige = GigESystem::new().unwrap();
                    gige.discover_cameras().await.unwrap();
                    gige.configure_cameras(sync_mode).await.unwrap();
                    gige.start_acquisition().await.unwrap();
                    
                    // Acquérir quelques images
                    for _ in 0..5 {
                        let frames = black_box(gige.acquire_frames().await.unwrap());
                        black_box(frames);
                    }
                    
                    // Arrêter l'acquisition
                    gige.stop_acquisition().await.unwrap();
                });
            });
        });
    }
    
    group.finish();
}

criterion_group!(benches, bench_acquisition, bench_sync_modes);
criterion_main!(benches);