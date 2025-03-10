#!/usr/bin/env python3
# heimdall/test_contamination.py

# Importation des bibliothèques standard
import os
import time
import logging
import numpy as np  # pour le traitement de tableaux numériques
import cv2  # OpenCV pour le traitement d'images

# Importation des composants Heimdall
from heimdall.core.acquisition import SimulationImageSource
from heimdall.inspection.contamination_inspector import ContaminationInspector

# Configuration du système de journalisation (logs)
logging.basicConfig(
    level=logging.INFO,  # Niveau de détail des logs (INFO est un bon équilibre)
    format="%(asctime)s - %(name)s - %(levelname)s - %(message)s"  # Format des messages de log
)

def add_artificial_contamination(image):
    """
    Ajoute des contaminations artificielles simples (points noirs/gris) à l'image
    
    Args:
        image: Image originale
        
    Returns:
        Image avec contaminations ajoutées
    """
    # Créer une copie de l'image pour ne pas modifier l'original
    result = image.copy()
    height, width = result.shape[:2]  # Récupérer les dimensions de l'image
    
    # Ajouter exactement 2 contaminations (points noirs ou gris)
    for i in range(2):
        # Position du point à un endroit visible dans l'image
        x = width // 4 + i * width // 2  # Répartir les points horizontalement
        y = height // 2                  # Milieu de l'image verticalement
        
        # Taille assez grande pour être bien visible
        size = np.random.randint(15, 30)  # Taille aléatoire entre 15 et 30 pixels
        
        # Couleur sombre (noir ou gris foncé)
        color_value = np.random.randint(0, 60)  # Valeur RGB entre 0 (noir) et 60 (gris foncé)
        color = (color_value, color_value, color_value)
        
        # Dessiner un cercle rempli qui représente la contamination
        cv2.circle(result, (x, y), size, color, -1)  # -1 signifie remplir le cercle
        
        print(f"Ajout de contamination #{i+1} à ({x}, {y}) avec taille {size} et couleur {color}")
    
    return result

def test_contamination_inspection():
    """Teste l'inspection de contamination avec des images simulées"""
    print("\n=== Test de l'inspection de contamination ===")
    
    # 1. Créer une source d'images simulées
    config = {
        "width": 640,           # Largeur de l'image
        "height": 480,          # Hauteur de l'image
        "pattern": "bottle",    # Génère des images de bouteilles
        "inject_defects": False  # Ne pas ajouter de défauts automatiques (on le fait manuellement)
    }
    source = SimulationImageSource("test_source", config)
    source.open()  # Ouvrir la source d'images
    
    # 2. Créer l'inspecteur de contamination
    inspector = ContaminationInspector("test_contamination_inspector")
    
    # 3. Traiter plusieurs images
    results = []
    for i in range(3):  # Traiter 3 images
        print(f"\nTraitement de l'image {i+1}...")
        
        # Lire une image depuis la source
        success, image = source.read()
        if success:
            # Ajouter des contaminations artificielles
            contaminated_image = add_artificial_contamination(image)
            
            # Enregistrer l'image pour inspection visuelle si nécessaire
            cv2.imwrite(f"contaminated_{i+1}.jpg", contaminated_image)
            
            # Inspecter l'image avec notre détecteur
            result = inspector.inspect(contaminated_image)
            results.append(result)
            
            # Afficher les résultats
            print(f"  Résultat d'inspection: {result}")
            print(f"  Défauts trouvés: {result.defect_count}")
            print(f"  Temps de traitement: {result.processing_time:.3f}s")
            
            # Liste des défauts détectés
            if result.defects:
                for idx, defect in enumerate(result.defects):
                    print(f"  Défaut #{idx+1}: {defect}")
            else:
                print("  Aucun défaut détecté!")
            
            # Afficher les images (résultats visuels)
            cv2.imshow("Image originale", result.images["original"])
            cv2.imshow("Image traitée", result.images["processed"])
            
            if "visualization" in result.images:
                cv2.imshow("Visualisation", result.images["visualization"])
                
            # Attendre 3 secondes avant de continuer
            # Utiliser 3000 ms au lieu de 0 pour éviter de bloquer indéfiniment
            cv2.waitKey(3000)
    
    # Fermer proprement les ressources
    source.close()
    cv2.destroyAllWindows()
    
    # Résumé des résultats
    defect_counts = [r.defect_count for r in results]
    if results:
        avg_processing_time = sum(r.processing_time for r in results) / len(results)
        print("\nRésumé du test:")
        print(f"  Images traitées: {len(results)}")
        print(f"  Images avec défauts: {sum(1 for r in results if r.has_defects)}")
        print(f"  Total des défauts trouvés: {sum(defect_counts)}")
        print(f"  Défauts moyens par image: {sum(defect_counts)/len(results):.1f}")
        print(f"  Temps de traitement moyen: {avg_processing_time:.3f}s")

if __name__ == "__main__":
    test_contamination_inspection()
    print("\nTest d'inspection de contamination terminé avec succès!")
