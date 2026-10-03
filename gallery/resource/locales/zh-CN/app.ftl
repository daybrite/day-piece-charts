# day-piece-charts 图库的字符串：私有目录，任何展示这些页面的应用无需安装即可读取。

nav_pipeline = 处理流程

# 图表组合选择器（src/pipeline.rs `Composition`）。
composition = 组合
comp_grouped = 分组柱形
comp_line = 折线
comp_stacked = 堆叠柱形
comp_donut = 环形
comp_heatmap = 热力图

# 图表自身的标签。
revenue = 收入（千）

# 读数：处理流程在绘制前推导出的内容。
facts_section = 处理流程推导的内容
fact_grammar = 语法
fact_marks = 标记
fact_series = 系列
fact_domain = 数据范围
fact_ticks = Y 轴刻度
data_section = 数据
months = 月数

# 图库：README 中记录的示例图表。
ex_line = 折线图
ex_line_series = 带目标的折线系列
ex_area = 渐变面积图
ex_area_stacked = 堆叠面积图
ex_bar = 带标签的柱形图
ex_bar_grouped = 分组柱形图
ex_bar_normalized = 百分比堆叠
ex_bar_horizontal = 条形图
ex_pie = 饼图与环形图
ex_scatter = 散点图
ex_heatmap = 热力网格
ex_time = 时间轴
ex_sparkline = 迷你图

ex_hover = 悬停或轻点图表以读取数值

# 每个图表上方的控件。每次更改都有动画。
ex_randomize = 随机数据
ex_sort = 按数值排序
ex_timeframe = 时间范围
ex_tf_half = 6 个月
ex_tf_year = 12 个月
ex_quarters = 季度
ex_slices = 扇区
ex_range = 范围

fact_selection = 选中
fact_selection_none = 悬停或轻点绘图区
ex_curve = 曲线
ex_curve_linear = 线性
ex_curve_monotone = 单调
ex_curve_catmull = Catmull-Rom
ex_curve_cardinal = 基数样条
ex_curve_step_start = 阶梯（起点）
ex_curve_step_center = 阶梯（中点）
ex_curve_step_end = 阶梯（终点）
ex_points = 数据点
ex_hole = 孔径 (%)
ex_stacking = 堆叠方式
ex_stack_standard = 堆叠
ex_stack_normalized = 100%
ex_stack_center = 流图
ex_stack_none = 重叠
ex_corners = 圆角
ex_stacked = 堆叠
ex_symbol_size = 大小
ex_palette = 配色
ex_palette_sequential = Viridis
ex_palette_diverging = 发散
ex_count = 数量
ex_series = 系列
ex_distribution = 分布
ex_dist_trend = 趋势
ex_dist_uniform = 均匀
ex_dist_normal = 正态簇
ex_dist_exponential = 指数
ex_dist_wave = 波形
ex_dist_fan = 扇形

# 应用将这些页面作为单个页面展示时（`gallery()`）页面上方的选择器。
gallery_page = 图表

# Declarative interaction examples
inter_linked = 联动点选择
inter_brush = 框选与交叉筛选
inter_overview = 概览与详情
inter_legend = 共享图例选择
inter_cells = 交互式热力图
inter_viewport = 平移和缩放
inter_compose = 组合选择条件
inter_links = 注册图表链接
inter_linked_note = 点击任一视图中的点。按住 Shift 点击可添加或移除点；相同标识关联两种编码。
inter_brush_note = 拖动矩形选择观测值。汇总仅统计选中的观测值。在选框内拖动可移动选区。 点击或轻触选区外部可清除选择。
inter_overview_note = 在概览中拖动选择水平范围。详情视图将此范围用作坐标域。 点击或轻触选区外部可清除选择。
inter_legend_note = 点击扇形、点或任一图例来切换分组。同一参数协调所有视图。
inter_cells_note = 悬停、点击或拖动浏览单元格以查看数据。选中的单元格保持高亮。
inter_viewport_note = 拖动平移，使用触控板或双指捏合缩放，也可使用缩放按钮。重置会恢复原始范围。
inter_compose_note = 在上方视图中框选，并在图例中选择分组。上方显示交集，下方显示并集。 点击或轻触选区外部可清除选择。
inter_links_note = 点击扇形或图例链接。两者使用同一注册处理程序，在下方显示路由。
inter_reset = 重置选择
inter_zoom_in = 放大
inter_zoom_out = 缩小
inter_group_a = 分组甲
inter_group_b = 分组乙
inter_group_c = 分组丙
inter_x = 测量 X
inter_y = 测量 Y
inter_count_axis = 观测值
inter_count = 已选择 { NUMBER($count, maximumFractionDigits: 0) } 个观测值
inter_band = 分区 { NUMBER($band, maximumFractionDigits: 0) }
inter_cell = 分区 { NUMBER($band, maximumFractionDigits: 0) } · { NUMBER($count, maximumFractionDigits: 0) } 个观测值
inter_link_target = 已激活路由：{ $target }

inter_select_sample = 选择一个观测值

inter_filtered = 联动数据筛选
inter_inputs = 输入绑定选择
inter_filtered_note = 在上方视图中框选。下方视图使用同一条件筛选数据，并保留原始坐标轴。 点击或轻触选区外部可清除选择。
inter_inputs_note = 在选择器、图表或图例中选择分组。基于字段的选择条件保持三个控件同步。
inter_all_groups = 所有分组
