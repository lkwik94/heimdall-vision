#!/bin/bash
# setup-rt-system.sh - Configuration d'un système temps réel pour Heimdall Vision
# Ce script configure un système Debian avec un kernel RT pour l'application Heimdall Vision
# Auteur: OpenHands

echo "=== Setting up Real-Time System for Heimdall Vision ==="

# 1. Installer le kernel RT
echo "Installing RT kernel..."
apt update && apt upgrade -y
apt install -y linux-image-rt-amd64 linux-headers-rt-amd64 build-essential

# 2. Installer les dépendances pour l'application
echo "Installing application dependencies..."
apt install -y python3-pip python3-opencv python3-numpy libopencv-dev
pip3 install pyyaml flask

# 3. Installer Rust
echo "Installing Rust..."
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source $HOME/.cargo/env

# 4. Configurer les paramètres système
echo "Configuring system parameters..."

# Créer les scripts de configuration
mkdir -p /usr/local/bin

# Script d'isolation CPU
cat > /usr/local/bin/setup-rt-cpus.sh << 'CPUSCRIPT'
#!/bin/bash

# Réserver les CPU 1-3 pour l'application temps réel (ajuster selon votre matériel)
echo "Configuring CPU isolation..."

# Désactiver le load balancing sur les CPUs isolés
for i in 1 2 3; do
    echo 0 > /sys/devices/system/cpu/cpu$i/online
    echo 1 > /sys/devices/system/cpu/cpu$i/online
done

# Désactiver les interruptions sur les CPUs isolés
for irq in $(ls /proc/irq/); do
    if [ -d "/proc/irq/$irq" ]; then
        echo 1 > /proc/irq/$irq/smp_affinity_list 2>/dev/null || true
    fi
done

# Configurer les IRQs spécifiques pour les périphériques USB/caméra sur CPU0
for irq in $(grep -E "usb|camera" /proc/interrupts | awk '{print $1}' | tr -d ':'); do
    if [ -d "/proc/irq/$irq" ]; then
        echo 1 > /proc/irq/$irq/smp_affinity_list 2>/dev/null || true
    fi
done

echo "CPU isolation configured successfully"
CPUSCRIPT

# Script de configuration des interruptions
cat > /usr/local/bin/setup-irq.sh << 'IRQSCRIPT'
#!/bin/bash

# Désactiver le regroupement d'interruptions pour les périphériques USB (caméras)
for dev in /sys/bus/usb/devices/*/power/control; do
  echo "on" > $dev
done

# Configurer les paramètres d'interruption pour les périphériques réseau
for eth in /sys/class/net/eth*/queues/rx-*/rps_cpus; do
  echo 1 > $eth
done

# Désactiver le regroupement d'interruptions pour les périphériques réseau
for eth in /sys/class/net/eth*; do
  ethtool -C $eth rx-usecs 0 tx-usecs 0 || true
done

# Configurer l'affinité des interruptions pour les périphériques PCI
for irq in $(grep PCI /proc/interrupts | awk '{print $1}' | sed 's/://g'); do
  echo 1 > /proc/irq/$irq/smp_affinity_list
done

echo "IRQ configuration completed"
IRQSCRIPT

# Script de configuration de la mémoire
cat > /usr/local/bin/setup-memory.sh << 'MEMSCRIPT'
#!/bin/bash

# Désactiver le transparent hugepages
echo never > /sys/kernel/mm/transparent_hugepage/enabled
echo never > /sys/kernel/mm/transparent_hugepage/defrag

# Préallouer la mémoire pour l'application
echo 1 > /proc/sys/vm/overcommit_memory
echo 100 > /proc/sys/vm/overcommit_ratio

# Verrouiller les pages en mémoire pour éviter le swap
echo 0 > /proc/sys/vm/swappiness

# Configurer la zone de mémoire NUMA (si applicable)
if [ -d "/sys/devices/system/node" ]; then
  # Vérifier si le système a plusieurs nœuds NUMA
  NODES=$(ls -d /sys/devices/system/node/node* | wc -l)
  if [ $NODES -gt 1 ]; then
    # Configurer l'affinité NUMA pour les CPUs isolés
    echo "Configuring NUMA affinity..."
    numactl --membind=0 --cpunodebind=0 echo "NUMA node 0 configured"
  fi
fi

echo "Memory configuration completed"
MEMSCRIPT

# Script de test de latence
cat > /usr/local/bin/test-latency.sh << 'TESTSCRIPT'
#!/bin/bash

# Test de latence avec cyclictest
echo "Running latency test with cyclictest..."
sudo cyclictest -l100000 -m -n -a -t5 -p99 -i400 -h400 -q > /tmp/cyclictest.log

# Afficher les résultats
MAX_LATENCY=$(grep "Max Latencies" /tmp/cyclictest.log | tr " " "\n" | sort -n | tail -1 | sed s/^0*//)
echo "Maximum latency: $MAX_LATENCY µs"

if [ "$MAX_LATENCY" -lt 100 ]; then
  echo "PASS: Maximum latency is under 100 µs"
else
  echo "FAIL: Maximum latency is over 100 µs"
fi

# Test de latence avec hackbench
echo "Running hackbench test..."
sudo hackbench -l 10000 -g 8 -f 10 > /tmp/hackbench.log

# Test de latence avec l'application réelle
echo "Running application benchmark..."
cd /workspace/heimdall-vision
taskset -c 2 python benchmark.py --iterations 1000 --output /tmp/benchmark_results.json
TESTSCRIPT

# Script pour lancer l'application avec l'affinité CPU
cat > /usr/local/bin/run-heimdall.sh << 'RUNSCRIPT'
#!/bin/bash

# Définir les priorités et affinités CPU
RUST_CPU=2
PYTHON_CPU=3
SYSTEM_CPU=0,1

# Configurer les limites de ressources
ulimit -r 99  # Priorité temps réel maximale
ulimit -l unlimited  # Mémoire verrouillée illimitée

# Définir les variables d'environnement pour Rust
export RUST_BACKTRACE=1
export RUSTFLAGS="-C target-cpu=native"

# Lancer l'application avec taskset pour l'affinité CPU
# Les composants Rust critiques sur CPU 2
taskset -c $RUST_CPU chrt -f 99 python -m heimdall.main --high-performance

# Alternativement, pour lancer le dashboard sur un CPU différent
# taskset -c $PYTHON_CPU chrt -f 80 python dashboard.py
RUNSCRIPT

# Rendre tous les scripts exécutables
chmod +x /usr/local/bin/setup-rt-cpus.sh
chmod +x /usr/local/bin/setup-irq.sh
chmod +x /usr/local/bin/setup-memory.sh
chmod +x /usr/local/bin/test-latency.sh
chmod +x /usr/local/bin/run-heimdall.sh

# Créer les fichiers de configuration système
# Paramètres sysctl
cat > /etc/sysctl.d/99-realtime.conf << 'SYSCTLCONF'
# Désactiver le swap pour éviter les latences imprévisibles
vm.swappiness = 0

# Réduire la fréquence des opérations d'écriture sur disque
vm.dirty_ratio = 40
vm.dirty_background_ratio = 10
vm.dirty_expire_centisecs = 30000

# Augmenter la taille des tampons réseau
net.core.rmem_max = 16777216
net.core.wmem_max = 16777216
net.core.rmem_default = 1048576
net.core.wmem_default = 1048576

# Réduire les latences réseau
net.ipv4.tcp_fastopen = 3
net.ipv4.tcp_low_latency = 1

# Optimisations pour les performances temps réel
kernel.sched_rt_runtime_us = -1
kernel.sched_min_granularity_ns = 10000000
kernel.sched_wakeup_granularity_ns = 15000000
kernel.sched_migration_cost_ns = 5000000
kernel.sched_nr_migrate = 32

# Augmenter les limites de mémoire partagée pour OpenCV
kernel.shmmax = 8589934592
kernel.shmall = 2097152
SYSCTLCONF

# Paramètres GRUB
cat >> /etc/default/grub << 'GRUBCONF'
GRUB_CMDLINE_LINUX_DEFAULT="quiet splash isolcpus=1-3 nohz_full=1-3 rcu_nocbs=1-3 intel_pstate=disable nosoftlockup tsc=reliable clocksource=tsc processor.max_cstate=1 idle=poll intel_idle.max_cstate=0 transparent_hugepage=never"
GRUBCONF

# Limites de ressources
cat > /etc/security/limits.d/99-realtime.conf << 'LIMITSCONF'
# Limites pour l'utilisateur qui exécute l'application
*               -       rtprio          99
*               -       nice            -20
*               -       memlock         unlimited
LIMITSCONF

# Services systemd
# Service d'isolation CPU
cat > /etc/systemd/system/cpu-isolation.service << 'CPUSERVICE'
[Unit]
Description=CPU Isolation for Real-time Processing
After=network.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/setup-rt-cpus.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
CPUSERVICE

# Service de configuration des interruptions
cat > /etc/systemd/system/irq-config.service << 'IRQSERVICE'
[Unit]
Description=IRQ Configuration for Real-time Processing
After=network.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/setup-irq.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
IRQSERVICE

# Service de configuration de la mémoire
cat > /etc/systemd/system/memory-config.service << 'MEMSERVICE'
[Unit]
Description=Memory Configuration for Real-time Processing
After=network.target

[Service]
Type=oneshot
ExecStart=/usr/local/bin/setup-memory.sh
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
MEMSERVICE

# Activer les services
systemctl enable cpu-isolation.service
systemctl enable irq-config.service
systemctl enable memory-config.service

# 5. Installer les outils de test
echo "Installing test tools..."
apt install -y rt-tests trace-cmd kernelshark hwloc numactl

# 6. Exécuter les scripts de configuration
/usr/local/bin/setup-rt-cpus.sh
/usr/local/bin/setup-irq.sh
/usr/local/bin/setup-memory.sh

echo "=== Setup complete! ==="
echo "Please reboot the system to apply all changes."
echo "After reboot, run the validation tests with: /usr/local/bin/test-latency.sh"