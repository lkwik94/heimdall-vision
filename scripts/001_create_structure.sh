cd /root/dev/heimdall_systems/

# Créer la structure principale du projet (un dossier à la fois)
mkdir -p heimdall
mkdir -p heimdall/core
mkdir -p heimdall/inspection
mkdir -p heimdall/detectors
mkdir -p heimdall/camera
mkdir -p heimdall/communication
mkdir -p heimdall/utils
mkdir -p heimdall/ui
mkdir -p heimdall/ui/widgets

# Créer les fichiers __init__.py pour chaque module
touch heimdall/__init__.py
touch heimdall/core/__init__.py
touch heimdall/inspection/__init__.py
touch heimdall/detectors/__init__.py
touch heimdall/camera/__init__.py
touch heimdall/communication/__init__.py
touch heimdall/utils/__init__.py
touch heimdall/ui/__init__.py
touch heimdall/ui/widgets/__init__.py

# Créer les fichiers principaux
touch heimdall/main.py
touch heimdall/settings.py
