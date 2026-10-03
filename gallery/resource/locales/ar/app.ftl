# سلاسل معرض day-piece-charts: كتالوج خاص يقرؤه كل تطبيق يعرض هذه الصفحات دون أن يثبّته.

nav_pipeline = مسار المعالجة

# منتقي تركيبات المخطط (src/pipeline.rs `Composition`).
composition = التركيب
comp_grouped = أعمدة مجمّعة
comp_line = خطوط
comp_stacked = أعمدة مكدّسة
comp_donut = حلقة
comp_heatmap = خريطة حرارية

# تسميات المخطط نفسه.
revenue = الإيرادات (بالآلاف)

# الملخص: ما يستنتجه مسار المعالجة قبل الرسم.
facts_section = ما يستنتجه مسار المعالجة
fact_grammar = القواعد
fact_marks = العلامات
fact_series = السلاسل
fact_domain = نطاق البيانات
fact_ticks = تدريجات المحور Y
data_section = البيانات
months = الأشهر

# المعرض: المخططات التوضيحية التي يوثقها ملف README.
ex_line = مخطط خطي
ex_line_series = سلاسل خطية مع هدف
ex_area = مساحة متدرجة
ex_area_stacked = مساحات مكدّسة
ex_bar = مخطط أعمدة مع تسميات
ex_bar_grouped = مخطط أعمدة مجمّعة
ex_bar_normalized = مكدّس حتى ‎100%‎
ex_bar_horizontal = أعمدة أفقية
ex_pie = دائري وحلقي
ex_scatter = مخطط تبعثر
ex_heatmap = شبكة حرارية
ex_time = محور زمني
ex_sparkline = مخطط مصغّر

ex_hover = مرّر المؤشر أو المس المخطط لقراءة قيمة

# عناصر التحكم فوق كل مخطط. كل تغيير متحرك.
ex_randomize = بيانات عشوائية
ex_sort = الترتيب حسب القيمة
ex_timeframe = الفترة
ex_tf_half = ٦ أشهر
ex_tf_year = ١٢ شهرًا
ex_quarters = الأرباع
ex_slices = الشرائح
ex_range = المدى

fact_selection = التحديد
fact_selection_none = مرّر المؤشر أو المس منطقة الرسم
ex_curve = المنحنى
ex_curve_linear = خطي
ex_curve_monotone = رتيب
ex_curve_catmull = كاتمول-روم
ex_curve_cardinal = كاردينال
ex_curve_step_start = درجات (البداية)
ex_curve_step_center = درجات (الوسط)
ex_curve_step_end = درجات (النهاية)
ex_points = النقاط
ex_hole = الفتحة (%)
ex_stacking = التكديس
ex_stack_standard = مكدّس
ex_stack_normalized = ‎100%‎
ex_stack_center = تدفق
ex_stack_none = متراكب
ex_corners = الزوايا
ex_stacked = مكدّس
ex_symbol_size = الحجم
ex_palette = لوحة الألوان
ex_palette_sequential = فيريديس
ex_palette_diverging = متباعدة
ex_count = العدد
ex_series = السلاسل
ex_distribution = التوزيع
ex_dist_trend = اتجاه
ex_dist_uniform = منتظم
ex_dist_normal = تجمعات طبيعية
ex_dist_exponential = أسّي
ex_dist_wave = موجة
ex_dist_fan = مروحة

# المنتقي فوق الصفحات حين يعرضها تطبيق كصفحة واحدة (`gallery()`).
gallery_page = المخطط

# Declarative interaction examples
inter_linked = تحديد نقاط مترابطة
inter_brush = التحديد والتصفية المتقاطعة
inter_overview = نظرة عامة وتفاصيل
inter_legend = تحديد مشترك عبر وسيلة الإيضاح
inter_cells = خريطة حرارية تفاعلية
inter_viewport = التحريك والتكبير
inter_compose = دمج شروط التحديد
inter_links = روابط الرسم المسجلة
inter_linked_note = انقر نقطة في أي عرض. انقر مع Shift لإضافة نقاط أو إزالتها؛ تربط الهوية بين التمثيلين.
inter_brush_note = اسحب مستطيلاً لتحديد مشاهدات. يحسب الملخص تلك المشاهدات فقط. اسحب داخل التحديد لتحريكه. انقر أو المس خارج المنطقة المحددة لمسح التحديد.
inter_overview_note = حدد نطاقاً أفقياً في النظرة العامة. يستخدم عرض التفاصيل هذا النطاق لمقياسه. انقر أو المس خارج المنطقة المحددة لمسح التحديد.
inter_legend_note = انقر الشرائح أو النقاط أو وسيلة الإيضاح لتبديل المجموعات. ينسق المعامل نفسه بين العروض.
inter_cells_note = مرر المؤشر أو المس الخلايا أو اسحب فوقها لفحص البيانات. تبقى الخلية المحددة بارزة.
inter_viewport_note = اسحب للتحريك واستخدم إيماءة القرص أو أزرار التكبير. تعيد إعادة الضبط النطاقات الأصلية.
inter_compose_note = حدد منطقة في العرض العلوي ومجموعات في وسيلة الإيضاح. يعرض الأعلى تقاطعها والأسفل اتحادها. انقر أو المس خارج المنطقة المحددة لمسح التحديد.
inter_links_note = انقر شريحة أو رابطاً في وسيلة الإيضاح. يستخدمان المعالج نفسه ويظهر المسار أدناه.
inter_reset = إعادة ضبط التحديد
inter_zoom_in = تكبير
inter_zoom_out = تصغير
inter_group_a = المجموعة أ
inter_group_b = المجموعة ب
inter_group_c = المجموعة ج
inter_x = القياس س
inter_y = القياس ص
inter_count_axis = المشاهدات
inter_count = { NUMBER($count, maximumFractionDigits: 0) } مشاهدات محددة
inter_band = النطاق { NUMBER($band, maximumFractionDigits: 0) }
inter_cell = النطاق { NUMBER($band, maximumFractionDigits: 0) } · { NUMBER($count, maximumFractionDigits: 0) } مشاهدات
inter_link_target = المسار المنشط: { $target }

inter_select_sample = تحديد مشاهدة واحدة

inter_filtered = تصفية البيانات المترابطة
inter_inputs = تحديد مرتبط بعناصر التحكم
inter_filtered_note = حدد منطقة في العرض العلوي. يصفي العرض السفلي بياناته بالشرط نفسه مع الحفاظ على المحاور الأصلية. انقر أو المس خارج المنطقة المحددة لمسح التحديد.
inter_inputs_note = اختر مجموعة من القائمة أو الرسم أو وسيلة الإيضاح. يبقي شرط التحديد المعتمد على الحقل عناصر التحكم متزامنة.
inter_all_groups = كل المجموعات
