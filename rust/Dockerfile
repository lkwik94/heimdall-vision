FROM rust:1.74-slim-bullseye as builder

# Installer les dépendances
RUN apt-get update && apt-get install -y \
    build-essential \
    cmake \
    pkg-config \
    libopencv-dev \
    libaravis-dev \
    libglib2.0-dev \
    libusb-1.0-0-dev \
    && rm -rf /var/lib/apt/lists/*

# Créer un utilisateur non-root
RUN useradd -m -u 1000 -U -s /bin/bash heimdall

# Copier le code source
WORKDIR /heimdall
COPY --chown=heimdall:heimdall . .

# Compiler en mode release
USER heimdall
RUN cargo build --release --workspace

# Image finale
FROM debian:bullseye-slim

# Installer les dépendances runtime
RUN apt-get update && apt-get install -y \
    libopencv-dev \
    libaravis-0.8-0 \
    libglib2.0-0 \
    libusb-1.0-0 \
    && rm -rf /var/lib/apt/lists/*

# Créer un utilisateur non-root
RUN useradd -m -u 1000 -U -s /bin/bash heimdall

# Copier les binaires compilés
COPY --from=builder --chown=heimdall:heimdall /heimdall/target/release/heimdall-server /usr/local/bin/
COPY --from=builder --chown=heimdall:heimdall /heimdall/target/release/heimdall-cli /usr/local/bin/
COPY --from=builder --chown=heimdall:heimdall /heimdall/target/release/libheimdall.so /usr/local/lib/

# Configurer les permissions pour les caméras GigE
RUN echo 'SUBSYSTEM=="usb", ATTRS{idVendor}=="1ab2", MODE="0666"' > /etc/udev/rules.d/40-aravis.rules

# Configurer l'environnement
ENV LD_LIBRARY_PATH=/usr/local/lib:$LD_LIBRARY_PATH

# Définir le répertoire de travail
WORKDIR /heimdall
USER heimdall

# Exposer les ports
EXPOSE 8080 9090

# Point d'entrée
ENTRYPOINT ["heimdall-server"]