# Les chaînes de la galerie day-piece-charts : un catalogue privé, lu par toute app qui monte
# ces pages sans avoir à l'installer.

nav_pipeline = Pipeline

# Le sélecteur de compositions de graphiques (src/pipeline.rs `Composition`).
composition = Composition
comp_grouped = Barres groupées
comp_line = Lignes
comp_stacked = Barres empilées
comp_donut = Anneau
comp_heatmap = Carte de chaleur

# Les libellés propres au graphique.
revenue = Chiffre d'affaires (milliers)

# Le relevé : ce que le pipeline déduit avant de dessiner.
facts_section = Ce que le pipeline déduit
fact_grammar = Grammaire
fact_marks = Marques
fact_series = Séries
fact_domain = Plage des données
fact_ticks = Graduations Y

# Le contrôle des données.
data_section = Données
months = Mois

# La galerie : les graphiques d'illustration que documente le README de la crate.
ex_line = Graphique linéaire
ex_line_series = Séries linéaires avec objectif
ex_area = Aire en dégradé
ex_area_stacked = Aires empilées
ex_bar = Histogramme avec étiquettes
ex_bar_grouped = Barres groupées
ex_bar_normalized = Empilé à 100 %
ex_bar_horizontal = Barres horizontales
ex_pie = Secteurs et anneau
ex_scatter = Nuage de points
ex_heatmap = Grille de chaleur
ex_time = Axe temporel
ex_sparkline = Sparkline

# La ligne sous un graphique interactif, avant toute sélection.
ex_hover = Survolez ou touchez le graphique pour lire une valeur

# Les contrôles au-dessus de chaque graphique de la galerie. Chaque changement est animé.
ex_randomize = Données aléatoires
ex_sort = Trier par valeur
ex_timeframe = Période
ex_tf_half = 6 mois
ex_tf_year = 12 mois
ex_quarters = Trimestres
ex_slices = Parts
ex_range = Plage

# La ligne de sélection du relevé, et ce qu'elle affiche avant que le pointeur survole le tracé.
fact_selection = Sélection
fact_selection_none = Survolez ou touchez le tracé
ex_curve = Courbe
ex_curve_linear = Linéaire
ex_curve_monotone = Monotone
ex_curve_catmull = Catmull-Rom
ex_curve_cardinal = Cardinale
ex_curve_step_start = Palier (début)
ex_curve_step_center = Palier (centre)
ex_curve_step_end = Palier (fin)
ex_points = Points
ex_hole = Trou (%)
ex_stacking = Empilement
ex_stack_standard = Empilé
ex_stack_normalized = 100 %
ex_stack_center = Flux
ex_stack_none = Superposé
ex_corners = Coins
ex_stacked = Empilé
ex_symbol_size = Taille
ex_palette = Palette
ex_palette_sequential = Viridis
ex_palette_diverging = Divergente
ex_count = Nombre
ex_series = Séries
ex_distribution = Distribution
ex_dist_trend = Tendance
ex_dist_uniform = Uniforme
ex_dist_normal = Groupes normaux
ex_dist_exponential = Exponentielle
ex_dist_wave = Onde
ex_dist_fan = Éventail

# Le sélecteur au-dessus des pages quand une app les montre en une seule (`gallery()`).
gallery_page = Graphique

# Declarative interaction examples
inter_linked = Sélections de points liées
inter_brush = Brossage et filtrage croisé
inter_overview = Vue générale et détails
inter_legend = Sélections de légende partagées
inter_cells = Carte thermique interactive
inter_viewport = Déplacer et zoomer
inter_compose = Combiner les prédicats de sélection
inter_links = Liens de graphique enregistrés
inter_linked_note = Cliquez sur un point dans une vue. Maj-clic ajoute ou retire des points ; leur identité relie les deux représentations.
inter_brush_note = Tracez un rectangle pour sélectionner des observations. Le résumé ne compte que celles-ci. Déplacez la sélection en glissant à l’intérieur. Cliquez ou touchez en dehors de la zone sélectionnée pour l’effacer.
inter_overview_note = Sélectionnez une plage horizontale dans la vue générale. La vue détaillée utilise cette plage comme domaine. Cliquez ou touchez en dehors de la zone sélectionnée pour l’effacer.
inter_legend_note = Cliquez sur les secteurs, points ou légendes pour sélectionner des groupes. Le même paramètre coordonne toutes les vues.
inter_cells_note = Survolez, touchez ou parcourez les cellules pour examiner leurs données. La cellule sélectionnée reste lumineuse.
inter_viewport_note = Glissez pour déplacer, pincez pour zoomer, ou utilisez les boutons. Réinitialiser restaure les domaines initiaux.
inter_compose_note = Sélectionnez une zone en haut et des groupes dans la légende. En haut : leur intersection ; en bas : leur union. Cliquez ou touchez en dehors de la zone sélectionnée pour l’effacer.
inter_links_note = Cliquez sur un secteur ou un lien de légende. Le même gestionnaire affiche la route ci-dessous.
inter_reset = Réinitialiser la sélection
inter_zoom_in = Zoom avant
inter_zoom_out = Zoom arrière
inter_group_a = Groupe A
inter_group_b = Groupe B
inter_group_c = Groupe C
inter_x = Mesure X
inter_y = Mesure Y
inter_count_axis = Observations
inter_count = { NUMBER($count, maximumFractionDigits: 0) } observations sélectionnées
inter_band = Bande { NUMBER($band, maximumFractionDigits: 0) }
inter_cell = Bande { NUMBER($band, maximumFractionDigits: 0) } · { NUMBER($count, maximumFractionDigits: 0) } observations
inter_link_target = Route activée : { $target }

inter_select_sample = Sélectionner une observation

inter_filtered = Filtrage de données lié
inter_inputs = Sélections liées aux contrôles
inter_filtered_note = Sélectionnez une zone en haut. Le bas filtre ses données avec la même requête et conserve les axes initiaux. Cliquez ou touchez en dehors de la zone sélectionnée pour l’effacer.
inter_inputs_note = Choisissez un groupe dans le sélecteur, le graphique ou la légende. Une requête sur un champ synchronise les trois contrôles.
inter_all_groups = Tous les groupes
