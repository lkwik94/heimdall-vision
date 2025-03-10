# heimdall/detectors/contamination_detector.py

# Importation des bibliothèques nécessaires
import numpy as np
import cv2
from typing import Dict, List, Any, Optional, Tuple

# Import de nos classes de base pour les détecteurs
from heimdall.detectors.base import DefectDetector, Defect

class ContaminationDetector(DefectDetector):
    """Détecteur d'impuretés et contaminations dans les bouteilles"""
    
    def __init__(self, name: str = "contamination_detector", config: Dict[str, Any] = None):
        """
        Initialise le détecteur de contamination
        
        Args:
            name: Nom du détecteur
            config: Paramètres de configuration
        """
        super().__init__(name, config)
        
        # Extraction des paramètres de configuration avec valeurs par défaut
        # Taille minimale d'une impureté pour être considérée (en pixels carrés)
        self.min_contaminant_size = self.config.get("min_contaminant_size", 10)
        
        # Taille maximale d'une impureté (pour éviter de détecter de grandes zones)
        self.max_contaminant_size = self.config.get("max_contaminant_size", 3000)
        
        # Seuil de contraste pour la détection (plus basse = plus sensible)
        self.contrast_threshold = self.config.get("contrast_threshold", 15)
        
        # Confiance minimale pour considérer un défaut (entre 0 et 1)
        self.min_confidence = self.config.get("min_confidence", 0.25)
        
        # Activer l'analyse de couleur en plus de l'analyse de luminosité
        self.use_color = self.config.get("use_color", True)
        
        # Log des paramètres pour le débogage
        self.logger.info(f"Détecteur {self.name} initialisé avec min_size={self.min_contaminant_size}, "
                       f"max_size={self.max_contaminant_size}, threshold={self.contrast_threshold}")
        
    def detect(self, image: np.ndarray, context: Dict[str, Any] = None) -> List[Defect]:
        """
        Détecte les impuretés dans une image
        
        Args:
            image: Image d'entrée (couleur ou niveaux de gris)
            context: Informations contextuelles supplémentaires
            
        Returns:
            Liste des impuretés détectées
        """
        # Conserver l'original pour l'analyse de couleur si nécessaire
        original = image.copy()
        
        # Conversion en niveaux de gris si l'image est en couleur
        if len(image.shape) == 3:
            gray = cv2.cvtColor(image, cv2.COLOR_BGR2GRAY)
        else:
            gray = image
            
        # 1. Prétraitement pour améliorer la détection des impuretés
        # Appliquer un flou pour réduire le bruit
        blurred = cv2.GaussianBlur(gray, (5, 5), 0)
        
        # Utiliser un seuillage adaptatif pour gérer l'éclairage non uniforme
        # Le seuillage inversé (THRESH_BINARY_INV) fait ressortir les zones sombres
        binary = cv2.adaptiveThreshold(
            blurred,               # Image d'entrée 
            255,                   # Valeur maximale
            cv2.ADAPTIVE_THRESH_GAUSSIAN_C,  # Méthode adaptative
            cv2.THRESH_BINARY_INV,  # Zones sombres = 255, fond = 0
            11,                    # Taille de la fenêtre d'analyse
            self.contrast_threshold # Constante de seuil
        )
        
        # 2. Nettoyage morphologique pour améliorer la segmentation
        # Créer un noyau pour les opérations morphologiques
        kernel = cv2.getStructuringElement(cv2.MORPH_RECT, (3, 3))
        
        # OPEN élimine les petits points de bruit (érosion puis dilatation)
        binary = cv2.morphologyEx(binary, cv2.MORPH_OPEN, kernel)
        
        # CLOSE ferme les petits trous dans les objets (dilatation puis érosion)
        binary = cv2.morphologyEx(binary, cv2.MORPH_CLOSE, kernel)
        
        # 3. Détection de contours des zones potentiellement contaminées
        contours, _ = cv2.findContours(binary, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
        
        # 4. Filtrage et analyse des contours
        defects = []  # Liste pour stocker les défauts détectés
        
        for contour in contours:
            # Calculer l'aire (surface en pixels carrés)
            area = cv2.contourArea(contour)
            
            # Ignorer si trop petit ou trop grand
            if area < self.min_contaminant_size or area > self.max_contaminant_size:
                continue
                
            # Créer un masque pour ce contour pour analyser l'intensité
            mask = np.zeros_like(gray)
            cv2.drawContours(mask, [contour], 0, 255, -1)  # Dessiner le contour rempli
            
            # Calculer le rectangle englobant pour l'analyse de région
            x, y, w, h = cv2.boundingRect(contour)
            
            # Calculer le centre à partir des moments
            M = cv2.moments(contour)
            if M["m00"] > 0:  # Éviter la division par zéro
                cx = int(M["m10"] / M["m00"])  # Coordonnée x du centre
                cy = int(M["m01"] / M["m00"])  # Coordonnée y du centre
                position = (cx, cy)
                
                # 5. Calcul de confiance basé sur plusieurs facteurs
                
                # 5.1 Différence de contraste (plus grande = plus grande confiance)
                # Extraire la région d'intérêt
                roi = gray[y:y+h, x:x+w]  # Sous-image de la zone du défaut
                roi_mask = mask[y:y+h, x:x+w]  # Masque correspondant
                
                # Calculer l'intensité moyenne de l'arrière-plan et du premier plan
                background = np.mean(roi[roi_mask == 0]) if np.any(roi_mask == 0) else 127
                foreground = np.mean(roi[roi_mask == 255]) if np.any(roi_mask == 255) else 127
                intensity_diff = abs(background - foreground)
                
                # Normaliser le score d'intensité (0-1)
                intensity_score = min(1.0, intensity_diff / 30.0)
                
                # 5.2 Régularité de forme (plus irrégulier = probable contamination)
                # Comparer l'aire au rectangle englobant
                rect_area = w * h
                area_ratio = area / rect_area if rect_area > 0 else 0
                shape_score = 1.0 - area_ratio  # Ratio plus bas = forme plus irrégulière
                
                # 5.3 Analyse de couleur (si activée et image couleur disponible)
                color_score = 0.5  # Score neutre par défaut
                if self.use_color and len(original.shape) == 3:
                    # Extraire les statistiques de couleur de la région
                    roi_color = original[y:y+h, x:x+w]
                    roi_color_mask = mask[y:y+h, x:x+w]
                    
                    # Vérifier les différences significatives dans chaque canal
                    channels = cv2.split(roi_color)  # Séparer les canaux RGB/BGR
                    channel_diffs = []
                    
                    for channel in channels:
                        fg = np.mean(channel[roi_color_mask == 255]) if np.any(roi_color_mask == 255) else 127
                        bg = np.mean(channel[roi_color_mask == 0]) if np.any(roi_color_mask == 0) else 127
                        channel_diffs.append(abs(fg - bg))
                    
                    # Une variance de couleur plus élevée = probabilité plus élevée de contamination
                    color_diff = max(channel_diffs)
                    color_score = min(1.0, color_diff / 30.0)
                
                # Combiner les scores avec des poids
                # Intensité et couleur ont plus d'importance que la forme
                confidence = (intensity_score * 0.5) + (shape_score * 0.2) + (color_score * 0.3)
                
                # Ajouter seulement si la confiance est supérieure au seuil
                if confidence >= self.min_confidence:
                    defect = Defect(
                        defect_type="contamination",  # Type de défaut
                        position=position,            # Position (x,y)
                        size=area,                    # Taille en pixels
                        confidence=confidence,        # Score de confiance (0-1)
                        metadata={                    # Métadonnées supplémentaires
                            "intensity_diff": intensity_diff,
                            "shape_score": shape_score,
                            "color_score": color_score,
                            "bounding_box": (x, y, w, h),
                            "contour": contour.tolist()  # Conversion en liste pour sérialisation
                        }
                    )
                    defects.append(defect)
        
        # Ajout de logs détaillés pour le débogage
        self.logger.info(f"Analysé {len(contours)} contaminants potentiels")
        for i, contour in enumerate(contours):
            area = cv2.contourArea(contour)
            self.logger.info(f"Contour {i}: aire={area}, min={self.min_contaminant_size}, max={self.max_contaminant_size}")
            
            # Analyser uniquement si la taille est appropriée
            if area < self.min_contaminant_size or area > self.max_contaminant_size:
                continue
                
            # Calcul des métriques pour le débogage
            M = cv2.moments(contour)
            if M["m00"] > 0:
                cx = int(M["m10"] / M["m00"])
                cy = int(M["m01"] / M["m00"])
                
                # Préparation pour le calcul des scores
                mask = np.zeros_like(gray)
                cv2.drawContours(mask, [contour], 0, 255, -1)
                x, y, w, h = cv2.boundingRect(contour)
                roi = gray[y:y+h, x:x+w]
                roi_mask = mask[y:y+h, x:x+w]
                background = np.mean(roi[roi_mask == 0]) if np.any(roi_mask == 0) else 127
                foreground = np.mean(roi[roi_mask == 255]) if np.any(roi_mask == 255) else 127
                intensity_diff = abs(background - foreground)
                rect_area = w * h
                area_ratio = area / rect_area if rect_area > 0 else 0
                shape_score = 1.0 - area_ratio
                intensity_score = min(1.0, intensity_diff / 30.0)
                color_score = 0.5
                confidence = (intensity_score * 0.5) + (shape_score * 0.2) + (color_score * 0.3)
                
                # Log des valeurs calculées
                self.logger.info(f"Contour {i}: position=({cx},{cy}), confiance={confidence:.2f}, min_confiance={self.min_confidence}")
                self.logger.info(f"  Scores: intensité={intensity_score:.2f}, forme={shape_score:.2f}, couleur={color_score:.2f}")
        
        self.logger.debug(f"Trouvé {len(defects)} points de contamination dans l'image")
        return defects
    
    def visualize(self, image: np.ndarray, defects: List[Defect]) -> np.ndarray:
        """
        Visualise les contaminations détectées sur l'image
        
        Args:
            image: Image originale
            defects: Liste des défauts détectés
            
        Returns:
            Image avec défauts visualisés
        """
        # Créer une copie pour ne pas modifier l'original
        if len(image.shape) == 2:
            # Convertir niveau de gris en couleur pour la visualisation
            viz_image = cv2.cvtColor(image, cv2.COLOR_GRAY2BGR)
        else:
            viz_image = image.copy()
            
        # Dessiner chaque défaut
        for defect in defects:
            if defect.defect_type == "contamination":
                # Récupérer le rectangle englobant depuis les métadonnées
                if "bounding_box" in defect.metadata:
                    x, y, w, h = defect.metadata["bounding_box"]
                    
                    # Couleur basée sur la confiance (vert à rouge)
                    green = int(255 * (1 - defect.confidence))
                    red = int(255 * defect.confidence)
                    color = (0, green, red)  # Format BGR
                    
                    # Dessiner le rectangle englobant
                    cv2.rectangle(viz_image, (x, y), (x+w, y+h), color, 2)
                    
                    # Dessiner le contour si disponible
                    if "contour" in defect.metadata:
                        contour = np.array(defect.metadata["contour"], dtype=np.int32)
                        cv2.drawContours(viz_image, [contour], 0, color, 2)
                    
                    # Afficher l'étiquette de confiance
                    cv2.putText(
                        viz_image, 
                        f"{defect.confidence:.2f}", 
                        (x, y - 5),
                        cv2.FONT_HERSHEY_SIMPLEX, 
                        0.5, 
                        color, 
                        1
                    )
                else:
                    # Solution de secours si le rectangle englobant n'est pas disponible
                    radius = int(np.sqrt(defect.size / np.pi))
                    
                    # Couleur basée sur la confiance
                    green = int(255 * (1 - defect.confidence))
                    red = int(255 * defect.confidence)
                    color = (0, green, red)
                    
                    # Dessiner un cercle
                    cv2.circle(viz_image, defect.position, radius, color, 2)
                    
                    # Afficher l'étiquette de confiance
                    cv2.putText(
                        viz_image, 
                        f"{defect.confidence:.2f}", 
                        (defect.position[0] - 20, defect.position[1] - radius - 5),
                        cv2.FONT_HERSHEY_SIMPLEX, 
                        0.5, 
                        color, 
                        1
                    )
                
        return viz_image
