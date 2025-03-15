# Configuration du Système Temps Réel pour Heimdall Vision

Ce document explique comment configurer un système Linux temps réel optimisé pour l'application Heimdall Vision, garantissant une latence maximale de 10ms pour le traitement d'images à haute cadence (100 000 bouteilles/heure).

## Prérequis

- Système Debian 12 (Bookworm) ou version ultérieure
- Droits d'administrateur (sudo)
- Au moins 4 cœurs CPU (idéalement 8+)
- 8 Go de RAM minimum (16 Go recommandés)

## Installation

1. Exécutez le script d'installation en tant que root:

```bash
sudo bash scripts/setup-rt-system.sh
```

2. Redémarrez le système pour appliquer toutes les modifications:

```bash
sudo reboot
```

3. Après le redémarrage, vérifiez que le kernel RT est bien utilisé:

```bash
uname -a
```

Vous devriez voir "PREEMPT_RT" dans la sortie.

## Validation des Performances

Pour valider les performances temps réel du système:

```bash
sudo /usr/local/bin/test-latency.sh
```

Ce script exécute plusieurs tests de latence et affiche les résultats. La latence maximale devrait être inférieure à 100 µs pour garantir le traitement en moins de 10ms.

## Exécution de l'Application

Pour exécuter l'application Heimdall Vision avec les optimisations temps réel:

```bash
/usr/local/bin/run-heimdall.sh
```

Ce script configure automatiquement:
- L'affinité CPU (composants Rust sur CPU dédié)
- Les priorités temps réel
- Les limites de ressources système

## Structure des Fichiers

- `/usr/local/bin/setup-rt-cpus.sh`: Configuration de l'isolation CPU
- `/usr/local/bin/setup-irq.sh`: Configuration des interruptions
- `/usr/local/bin/setup-memory.sh`: Configuration de la mémoire
- `/usr/local/bin/test-latency.sh`: Tests de latence
- `/usr/local/bin/run-heimdall.sh`: Exécution de l'application
- `/etc/sysctl.d/99-realtime.conf`: Paramètres noyau
- `/etc/security/limits.d/99-realtime.conf`: Limites de ressources

## Dépannage

### Latence Excessive

Si vous observez une latence excessive (>100 µs):

1. Vérifiez les services en cours d'exécution:
```bash
systemctl list-units --type=service --state=running
```

2. Désactivez les services non essentiels supplémentaires:
```bash
sudo systemctl stop <service_name>
sudo systemctl disable <service_name>
```

3. Vérifiez les interruptions système:
```bash
cat /proc/interrupts
```

4. Ajustez l'affinité des interruptions si nécessaire:
```bash
echo 1 > /proc/irq/<IRQ_NUMBER>/smp_affinity_list
```

### Problèmes de Performance de l'Application

Si l'application ne respecte pas la contrainte de 10ms:

1. Exécutez le benchmark pour identifier les goulots d'étranglement:
```bash
python benchmark.py --detailed
```

2. Augmentez la priorité des threads critiques:
```bash
sudo chrt -f 99 <PID>
```

3. Vérifiez l'utilisation des ressources en temps réel:
```bash
sudo trace-cmd record -e sched_switch -e irq_handler_entry -e irq_handler_exit
```

## Références

- [Documentation du kernel PREEMPT_RT](https://wiki.linuxfoundation.org/realtime/start)
- [Guide d'optimisation temps réel pour Linux](https://access.redhat.com/documentation/en-us/red_hat_enterprise_linux_for_real_time/8/html/tuning_guide/index)
- [Rust pour les applications temps réel](https://ferrous-systems.com/blog/embedded-rust-on-stable/)