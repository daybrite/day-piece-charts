# The day-piece-charts gallery's strings: a private catalog (https://daybrite.dev/docs/localization
# "Private catalogs"), so every app that mounts the pages reads them without installing them.

nav_pipeline = Pipeline

# The picker of chart compositions (src/pipeline.rs `Composition`).
composition = Composition
comp_grouped = Grouped bars
comp_line = Lines
comp_stacked = Stacked bars
comp_donut = Donut
comp_heatmap = Heat map

# The chart's own labels.
revenue = Revenue (thousands)

# The readout: what the pipeline derives before it draws.
facts_section = What the pipeline derives
fact_grammar = Grammar
fact_marks = Marks
fact_series = Series
fact_domain = Data range
fact_ticks = Y ticks

# The data control.
data_section = Data
months = Months

# The gallery: the illustrative charts the crate's README documents (src/examples.rs). Each name
# is a picker entry and the heading of the README section that prints the same chart's source.
ex_line = Line chart
ex_line_series = Line series with a target
ex_area = Area with a gradient
ex_area_stacked = Stacked areas
ex_bar = Bar chart with labels
ex_bar_grouped = Grouped bar chart
ex_bar_normalized = Stacked to 100%
ex_bar_horizontal = Horizontal bars
ex_pie = Pie and donut
ex_scatter = Scatter plot
ex_heatmap = Heat map grid
ex_time = Time axis
ex_sparkline = Sparkline

# The line under an interactive chart, before anything is selected.
ex_hover = Hover or tap the chart to read a value

# The controls above each gallery chart. Every change animates (README "Animation").
ex_randomize = Randomize data
ex_sort = Sort by value
ex_timeframe = Timeframe
ex_tf_half = 6 months
ex_tf_year = 12 months
ex_quarters = Quarters
ex_slices = Slices
ex_range = Range

# The readout's selection row, and what it shows before the pointer has been over the plot.
fact_selection = Selection
fact_selection_none = Hover or tap the plot
ex_curve = Curve
ex_curve_linear = Linear
ex_curve_monotone = Monotone
ex_curve_catmull = Catmull-Rom
ex_curve_cardinal = Cardinal
ex_curve_step_start = Step (start)
ex_curve_step_center = Step (center)
ex_curve_step_end = Step (end)
ex_points = Points
ex_hole = Hole %
ex_stacking = Stacking
ex_stack_standard = Stacked
ex_stack_normalized = 100%
ex_stack_center = Stream
ex_stack_none = Overlap
ex_corners = Corners
ex_stacked = Stacked
ex_symbol_size = Size
ex_palette = Palette
ex_palette_sequential = Viridis
ex_palette_diverging = Diverging
ex_count = Count
ex_series = Series
ex_distribution = Distribution
ex_dist_trend = Trend
ex_dist_uniform = Uniform
ex_dist_normal = Normal clusters
ex_dist_exponential = Exponential
ex_dist_wave = Wave
ex_dist_fan = Fan

# The picker above the pages when an app shows them as one page (`gallery()`).
gallery_page = Chart
