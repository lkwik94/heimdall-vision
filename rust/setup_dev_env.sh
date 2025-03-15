#!/bin/bash
set -e

echo "Configuration de l'environnement de développement Heimdall Vision..."

# Installer Rust
if ! command -v rustc &> /dev/null; then
    echo "Installation de Rust..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source $HOME/.cargo/env
fi

# Installer les composants Rust
rustup component add rustfmt clippy
rustup update

# Installer les outils de développement
cargo install cargo-edit cargo-watch cargo-expand cargo-llvm-cov cargo-criterion cargo-flamegraph

# Installer les dépendances système
if [ "$(uname)" == "Linux" ]; then
    if command -v apt-get &> /dev/null; then
        echo "Installation des dépendances sur Debian/Ubuntu..."
        sudo apt-get update
        sudo apt-get install -y build-essential cmake pkg-config \
            libopencv-dev libaravis-dev libglib2.0-dev libusb-1.0-0-dev \
            libgtk-3-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
            valgrind kcachegrind linux-tools-common linux-tools-generic
    elif command -v dnf &> /dev/null; then
        echo "Installation des dépendances sur Fedora..."
        sudo dnf install -y gcc gcc-c++ cmake pkgconfig \
            opencv-devel aravis-devel glib2-devel libusb-devel \
            gtk3-devel gstreamer1-devel gstreamer1-plugins-base-devel \
            valgrind kcachegrind perf
    fi
elif [ "$(uname)" == "Darwin" ]; then
    echo "Installation des dépendances sur macOS..."
    brew install opencv aravis glib libusb gtk+3 gstreamer gst-plugins-base
fi

# Configurer les permissions pour les caméras GigE (Linux uniquement)
if [ "$(uname)" == "Linux" ]; then
    echo "Configuration des permissions pour les caméras GigE..."
    sudo groupadd -f realtime
    sudo usermod -aG realtime $USER
    
    sudo tee /etc/udev/rules.d/40-aravis.rules > /dev/null << 'EOT'
# Aravis GigE Vision devices
SUBSYSTEM=="usb", ATTRS{idVendor}=="1ab2", MODE="0666"
# GigE Vision ethernet devices
SUBSYSTEM=="net", ACTION=="add", ATTR{address}=="aa:bb:cc:*", RUN+="/sbin/ip link set %k mtu 9000"
EOT
    
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    
    sudo tee /etc/security/limits.d/99-realtime.conf > /dev/null << 'EOT'
@realtime soft rtprio 99
@realtime hard rtprio 99
@realtime soft memlock unlimited
@realtime hard memlock unlimited
@realtime soft nice -20
@realtime hard nice -20
EOT
    
    sudo tee /etc/sysctl.d/99-realtime.conf > /dev/null << 'EOT'
kernel.sched_rt_runtime_us = 980000
kernel.shmmax = 8589934592
kernel.shmall = 8589934592
vm.swappiness = 10
EOT
    
    sudo sysctl -p /etc/sysctl.d/99-realtime.conf
fi

# Créer les répertoires de base pour les modules
mkdir -p heimdall-{core,camera,rt,ipc,server,cli,py}/src

echo "Configuration terminée! Veuillez vous déconnecter et vous reconnecter pour que les changements de groupe prennent effet."