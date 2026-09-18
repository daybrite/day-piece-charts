# Charts Demo UI strings (https://daybrite.dev/docs/localization). Add a locale by dropping a
# sibling folder (e.g. locales/fr/app.ftl) and translating.

app_title = Charts Demo

# The sidebar: the two groups of rows, and the name of this app's own page.
nav_demo = This demo
nav_pipeline = Pipeline
nav_examples = Examples

# The picker of chart compositions (src/lib.rs `Composition`).
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
ex_pie = Pie chart
ex_donut = Donut chart
ex_scatter = Scatter plot
ex_heatmap = Heat map grid
ex_time = Time axis
ex_sparkline = Sparkline

# The line under an interactive chart, before anything is selected.
ex_hover = Hover or tap the chart to read a value

# The readout's selection row, and what it shows before the pointer has been over the plot.
fact_selection = Selection
fact_selection_none = Hover or tap the plot
