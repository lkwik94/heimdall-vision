# Détecteur de défauts sur bouteilles

Ce module implémente un détecteur de défauts sur bouteilles en temps réel utilisant les composants Heimdall Vision.

## Fonctionnalités

- Détection de plusieurs types de défauts :
  - Contamination (corps étrangers)
  - Fissures
  - Déformations
  - Défauts de couleur
- Traitement d'images en temps réel
- Visualisation des défauts détectés
- Statistiques de détection

## Architecture

Le système utilise une architecture à deux tâches temps réel :

1. **Tâche d'acquisition** : Capture les images depuis une caméra à une fréquence fixe (10 Hz)
2. **Tâche de détection** : Traite les images et détecte les défauts de manière asynchrone

La communication entre les tâches se fait via une file d'attente temps réel optimisée pour éviter les allocations dynamiques.

## Algorithme de détection

L'algorithme de détection utilise les étapes suivantes :

1. Conversion en niveaux de gris
2. Application d'un flou gaussien pour réduire le bruit
3. Seuillage adaptatif pour isoler les anomalies
4. Détection de contours
5. Analyse des contours pour identifier les défauts
6. Classification des défauts selon leur forme et taille

## Utilisation

Pour compiler et exécuter le détecteur :

```bash
cd /chemin/vers/heimdall-vision/rust
cargo run --example detection/bottle_defect_detector
```

## Configuration

Le détecteur peut être configuré en modifiant les paramètres suivants :

- `threshold` : Seuil de détection (valeur par défaut : 30.0)
- `min_size` : Taille minimale du défaut en pixels (valeur par défaut : 10.0)
- `max_size` : Taille maximale du défaut en pixels (valeur par défaut : 1000.0)
- `sensitivity` : Sensibilité de détection (valeur par défaut : 0.8)

## Intégration

Ce module peut être intégré dans un système de production en :

1. Remplaçant la caméra simulée par une vraie caméra GigE Vision
2. Ajoutant une tâche de décision pour rejeter les bouteilles défectueuses
3. Connectant le système à un automate industriel via une interface réseau temps réel