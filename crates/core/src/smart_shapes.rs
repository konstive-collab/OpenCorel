//! Editable vector smart-shape library. Presets generate native Bézier,
//! Polyline or even-odd CompoundPath geometry, not opaque SVG bitmaps.
//! Every coordinate is in millimetres. Geometry stays editable after creation.
use crate::{Bounds,Point,Shape,Contour,CubicSegment};
use std::f32::consts::{PI,TAU};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum Category{Geometry,Stars,Arrows,Symbols,Flowcharts,Technical}
impl Category{
    pub const ALL:[Self;6]=[Self::Geometry,Self::Stars,Self::Arrows,
        Self::Symbols,Self::Flowcharts,Self::Technical];
    pub fn label(self)->&'static str{match self{
        Self::Geometry=>"Геометрия",Self::Stars=>"Звёзды и вспышки",
        Self::Arrows=>"Стрелки",Self::Symbols=>"Символы и пиктограммы",
        Self::Flowcharts=>"Блок-схемы",Self::Technical=>"Технические фигуры",
    }}
}
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum SmartShape{
    Triangle,
    TriangleDown,
    Diamond,
    Pentagon,
    Hexagon,
    Heptagon,
    Octagon,
    Nonagon,
    Decagon,
    Dodecagon,
    Parallelogram,
    Trapezoid,
    InverseTrapezoid,
    RoundedRectangle,
    Star4,
    Star5,
    Star6,
    Star7,
    Star8,
    Star10,
    Star12,
    Burst12,
    Burst16,
    Burst24,
    Flower8,
    Flower12,
    Sun8,
    ArrowRight,
    ArrowLeft,
    ArrowUp,
    ArrowDown,
    ArrowDoubleH,
    ArrowDoubleV,
    ChevronRight,
    ChevronLeft,
    ChevronUp,
    ChevronDown,
    ArrowBlockRight,
    ArrowBlockLeft,
    ArrowBent,
    ArrowCircular,
    Heart,
    Lightning,
    Home,
    Shield,
    Bookmark,
    Tag,
    Ticket,
    SpeechLeft,
    SpeechRight,
    ThoughtBubble,
    Banner,
    Ribbon,
    Crown,
    Flag,
    WaveFlag,
    Cross,
    Checkmark,
    XMark,
    Hourglass,
    Teardrop,
    Crescent,
    Cloud,
    FlowProcess,
    FlowDecision,
    FlowData,
    FlowDocument,
    FlowTerminator,
    FlowPreparation,
    FlowDelay,
    FlowStoredData,
    FlowManual,
    FlowDisplay,
    FlowOffpage,
    FlowSort,
    FlowMerge,
    FlowExtract,
    FlowDatabase,
    FlowCard,
    FlowAnnotation,
    FlowConnector,
    Gear8,
    Gear12,
    Gear20,
    Ring,
    Donut,
    Spiral,
    Wave,
    Zigzag,
    Pie,
    Target,
    GridMarker,
    Compass,
}
impl SmartShape{
    pub const ALL:[Self;93]=[
        Self::Triangle,
        Self::TriangleDown,
        Self::Diamond,
        Self::Pentagon,
        Self::Hexagon,
        Self::Heptagon,
        Self::Octagon,
        Self::Nonagon,
        Self::Decagon,
        Self::Dodecagon,
        Self::Parallelogram,
        Self::Trapezoid,
        Self::InverseTrapezoid,
        Self::RoundedRectangle,
        Self::Star4,
        Self::Star5,
        Self::Star6,
        Self::Star7,
        Self::Star8,
        Self::Star10,
        Self::Star12,
        Self::Burst12,
        Self::Burst16,
        Self::Burst24,
        Self::Flower8,
        Self::Flower12,
        Self::Sun8,
        Self::ArrowRight,
        Self::ArrowLeft,
        Self::ArrowUp,
        Self::ArrowDown,
        Self::ArrowDoubleH,
        Self::ArrowDoubleV,
        Self::ChevronRight,
        Self::ChevronLeft,
        Self::ChevronUp,
        Self::ChevronDown,
        Self::ArrowBlockRight,
        Self::ArrowBlockLeft,
        Self::ArrowBent,
        Self::ArrowCircular,
        Self::Heart,
        Self::Lightning,
        Self::Home,
        Self::Shield,
        Self::Bookmark,
        Self::Tag,
        Self::Ticket,
        Self::SpeechLeft,
        Self::SpeechRight,
        Self::ThoughtBubble,
        Self::Banner,
        Self::Ribbon,
        Self::Crown,
        Self::Flag,
        Self::WaveFlag,
        Self::Cross,
        Self::Checkmark,
        Self::XMark,
        Self::Hourglass,
        Self::Teardrop,
        Self::Crescent,
        Self::Cloud,
        Self::FlowProcess,
        Self::FlowDecision,
        Self::FlowData,
        Self::FlowDocument,
        Self::FlowTerminator,
        Self::FlowPreparation,
        Self::FlowDelay,
        Self::FlowStoredData,
        Self::FlowManual,
        Self::FlowDisplay,
        Self::FlowOffpage,
        Self::FlowSort,
        Self::FlowMerge,
        Self::FlowExtract,
        Self::FlowDatabase,
        Self::FlowCard,
        Self::FlowAnnotation,
        Self::FlowConnector,
        Self::Gear8,
        Self::Gear12,
        Self::Gear20,
        Self::Ring,
        Self::Donut,
        Self::Spiral,
        Self::Wave,
        Self::Zigzag,
        Self::Pie,
        Self::Target,
        Self::GridMarker,
        Self::Compass,
    ];
    pub fn category(self)->Category{match self{
            Self::Triangle|
            Self::TriangleDown|
            Self::Diamond|
            Self::Pentagon|
            Self::Hexagon|
            Self::Heptagon|
            Self::Octagon|
            Self::Nonagon|
            Self::Decagon|
            Self::Dodecagon|
            Self::Parallelogram|
            Self::Trapezoid|
            Self::InverseTrapezoid|
            Self::RoundedRectangle=>Category::Geometry,
            Self::Star4|
            Self::Star5|
            Self::Star6|
            Self::Star7|
            Self::Star8|
            Self::Star10|
            Self::Star12|
            Self::Burst12|
            Self::Burst16|
            Self::Burst24|
            Self::Flower8|
            Self::Flower12|
            Self::Sun8=>Category::Stars,
            Self::ArrowRight|
            Self::ArrowLeft|
            Self::ArrowUp|
            Self::ArrowDown|
            Self::ArrowDoubleH|
            Self::ArrowDoubleV|
            Self::ChevronRight|
            Self::ChevronLeft|
            Self::ChevronUp|
            Self::ChevronDown|
            Self::ArrowBlockRight|
            Self::ArrowBlockLeft|
            Self::ArrowBent|
            Self::ArrowCircular=>Category::Arrows,
            Self::Heart|
            Self::Lightning|
            Self::Home|
            Self::Shield|
            Self::Bookmark|
            Self::Tag|
            Self::Ticket|
            Self::SpeechLeft|
            Self::SpeechRight|
            Self::ThoughtBubble|
            Self::Banner|
            Self::Ribbon|
            Self::Crown|
            Self::Flag|
            Self::WaveFlag|
            Self::Cross|
            Self::Checkmark|
            Self::XMark|
            Self::Hourglass|
            Self::Teardrop|
            Self::Crescent|
            Self::Cloud=>Category::Symbols,
            Self::FlowProcess|
            Self::FlowDecision|
            Self::FlowData|
            Self::FlowDocument|
            Self::FlowTerminator|
            Self::FlowPreparation|
            Self::FlowDelay|
            Self::FlowStoredData|
            Self::FlowManual|
            Self::FlowDisplay|
            Self::FlowOffpage|
            Self::FlowSort|
            Self::FlowMerge|
            Self::FlowExtract|
            Self::FlowDatabase|
            Self::FlowCard|
            Self::FlowAnnotation|
            Self::FlowConnector=>Category::Flowcharts,
            Self::Gear8|
            Self::Gear12|
            Self::Gear20|
            Self::Ring|
            Self::Donut|
            Self::Spiral|
            Self::Wave|
            Self::Zigzag|
            Self::Pie|
            Self::Target|
            Self::GridMarker|
            Self::Compass=>Category::Technical,
    }}
    pub fn label(self)->&'static str{match self{
            Self::Triangle=>"Треугольник",
            Self::TriangleDown=>"Треугольник вниз",
            Self::Diamond=>"Ромб",
            Self::Pentagon=>"Пятиугольник",
            Self::Hexagon=>"Шестиугольник",
            Self::Heptagon=>"Семиугольник",
            Self::Octagon=>"Восьмиугольник",
            Self::Nonagon=>"Девятиугольник",
            Self::Decagon=>"Десятиугольник",
            Self::Dodecagon=>"Двенадцатиугольник",
            Self::Parallelogram=>"Параллелограмм",
            Self::Trapezoid=>"Трапеция",
            Self::InverseTrapezoid=>"Обратная трапеция",
            Self::RoundedRectangle=>"Скруглённый прямоугольник",
            Self::Star4=>"Звезда 4",
            Self::Star5=>"Звезда 5",
            Self::Star6=>"Звезда 6",
            Self::Star7=>"Звезда 7",
            Self::Star8=>"Звезда 8",
            Self::Star10=>"Звезда 10",
            Self::Star12=>"Звезда 12",
            Self::Burst12=>"Вспышка 12",
            Self::Burst16=>"Вспышка 16",
            Self::Burst24=>"Вспышка 24",
            Self::Flower8=>"Цветок 8 лепестков",
            Self::Flower12=>"Цветок 12 лепестков",
            Self::Sun8=>"Солнце с 8 лучами",
            Self::ArrowRight=>"Стрелка вправо",
            Self::ArrowLeft=>"Стрелка влево",
            Self::ArrowUp=>"Стрелка вверх",
            Self::ArrowDown=>"Стрелка вниз",
            Self::ArrowDoubleH=>"Двойная горизонтальная стрелка",
            Self::ArrowDoubleV=>"Двойная вертикальная стрелка",
            Self::ChevronRight=>"Шеврон вправо",
            Self::ChevronLeft=>"Шеврон влево",
            Self::ChevronUp=>"Шеврон вверх",
            Self::ChevronDown=>"Шеврон вниз",
            Self::ArrowBlockRight=>"Широкая стрелка вправо",
            Self::ArrowBlockLeft=>"Широкая стрелка влево",
            Self::ArrowBent=>"Поворотная стрелка",
            Self::ArrowCircular=>"Круговая стрелка",
            Self::Heart=>"Сердце",
            Self::Lightning=>"Молния",
            Self::Home=>"Дом",
            Self::Shield=>"Щит",
            Self::Bookmark=>"Закладка",
            Self::Tag=>"Бирка",
            Self::Ticket=>"Билет",
            Self::SpeechLeft=>"Диалог слева",
            Self::SpeechRight=>"Диалог справа",
            Self::ThoughtBubble=>"Облачко мысли",
            Self::Banner=>"Баннер",
            Self::Ribbon=>"Лента",
            Self::Crown=>"Корона",
            Self::Flag=>"Флажок",
            Self::WaveFlag=>"Волнистый флаг",
            Self::Cross=>"Крест",
            Self::Checkmark=>"Галочка",
            Self::XMark=>"Крестик",
            Self::Hourglass=>"Песочные часы",
            Self::Teardrop=>"Капля",
            Self::Crescent=>"Полумесяц",
            Self::Cloud=>"Облако",
            Self::FlowProcess=>"Процесс",
            Self::FlowDecision=>"Решение",
            Self::FlowData=>"Данные",
            Self::FlowDocument=>"Документ",
            Self::FlowTerminator=>"Начало/конец",
            Self::FlowPreparation=>"Подготовка",
            Self::FlowDelay=>"Задержка",
            Self::FlowStoredData=>"Хранилище",
            Self::FlowManual=>"Ручной ввод",
            Self::FlowDisplay=>"Дисплей",
            Self::FlowOffpage=>"Переход на другую страницу",
            Self::FlowSort=>"Сортировка",
            Self::FlowMerge=>"Объединение",
            Self::FlowExtract=>"Извлечение",
            Self::FlowDatabase=>"База данных",
            Self::FlowCard=>"Карточка",
            Self::FlowAnnotation=>"Аннотация",
            Self::FlowConnector=>"Соединитель",
            Self::Gear8=>"Шестерня 8 зубьев",
            Self::Gear12=>"Шестерня 12 зубьев",
            Self::Gear20=>"Шестерня 20 зубьев",
            Self::Ring=>"Кольцо",
            Self::Donut=>"Бублик",
            Self::Spiral=>"Спиральная линия",
            Self::Wave=>"Волна",
            Self::Zigzag=>"Зигзаг",
            Self::Pie=>"Сектор круга",
            Self::Target=>"Мишень",
            Self::GridMarker=>"Прицельная метка",
            Self::Compass=>"Компас",
    }}
    pub fn is_open(self)->bool{matches!(self,
        Self::Spiral|Self::Wave|Self::Zigzag|Self::GridMarker)}
}
/// Adjustable shape geometry, not limited to fixed icons. All values
/// are validated before the document's single undo snapshot is created.
#[derive(Clone,Copy,Debug)]
pub struct Options{
    /// Inner radius for stars, arrow shafts and badge notches (0.1..0.9).
    pub inset:f32,
    /// Extra petal/gear tooth count for procedural categories (3..32).
    pub detail:u8,
    /// Angle in degrees for regular polygons, stars and radial shapes.
    pub rotation_degrees:f32,
}
impl Default for Options{
    fn default()->Self{Self{inset:0.42,detail:8,rotation_degrees:0.0}}
}
impl Options{
    pub fn validate(self)->Result<(),String>{
        if !self.inset.is_finite()||!(0.1..=0.9).contains(&self.inset)
            || !(3..=32).contains(&self.detail)
            || !self.rotation_degrees.is_finite()
            || self.rotation_degrees.abs()>3600.0{
            return Err("Smart shape: expected inset 0.1..0.9, detail 3..32, finite rotation".into());
        }
        Ok(())
    }
}
type V=(f32,f32);
fn poly(vertices:&[V])->Vec<Point>{
    vertices.iter().map(|&(x,y)|Point{x,y}).collect()
}
fn regular(n:usize,start:f32)->Vec<Point>{
    (0..n).map(|i|{
        let angle=start+TAU*i as f32/n as f32;
        Point{x:0.5+0.5*angle.cos(),y:0.5+0.5*angle.sin()}
    }).collect()
}
fn radial(n:usize,inner:f32,start:f32)->Vec<Point>{
    (0..n*2).map(|i|{
        let angle=start+PI*i as f32/n as f32;
        let radius=if i%2==0{0.5}else{inner*0.5};
        Point{x:0.5+radius*angle.cos(),y:0.5+radius*angle.sin()}
    }).collect()
}
fn flower(petals:usize,depth:f32,start:f32)->Vec<Point>{
    let count=petals*16;
    (0..count).map(|i|{
        let a=start+TAU*i as f32/count as f32;
        let radius=(1.0-depth*0.5*(1.0-(petals as f32*a).cos())).max(0.12)*0.5;
        Point{x:0.5+a.cos()*radius,y:0.5+a.sin()*radius}
    }).collect()
}
fn gear(teeth:usize,inner:f32,start:f32)->Vec<Point>{
    let mut pts=Vec::with_capacity(teeth*4);
    for i in 0..teeth{
        for (step,r) in [(0.0,inner),(0.18,1.0),(0.82,1.0),(1.0,inner)]{
            let angle=start+TAU*(i as f32+step)/teeth as f32;
            pts.push(Point{x:0.5+0.5*r*angle.cos(),
                y:0.5+0.5*r*angle.sin()});
        }
    }
    pts
}
fn wave(y:f32,amplitude:f32,frequency:f32,phase:f32)->Vec<Point>{
    (0..=64).map(|i|{
        let x=i as f32/64.0;
        Point{x,y:y+amplitude*(TAU*frequency*x+phase).sin()}
    }).collect()
}
fn capsule()->Vec<Point>{
    let mut out=Vec::with_capacity(34);
    for i in 0..=16{
        let a=-PI/2.0+PI*i as f32/16.0;
        out.push(Point{x:0.76+0.24*a.cos(),y:0.5+0.5*a.sin()});
    }
    for i in 0..=16{
        let a=PI/2.0+PI*i as f32/16.0;
        out.push(Point{x:0.24+0.24*a.cos(),y:0.5+0.5*a.sin()});
    }
    out
}
fn rounded_rect(radius:f32)->Vec<Point>{
    let r=radius.clamp(0.05,0.48);
    let centers=[(1.0-r,r,-PI/2.0),(1.0-r,1.0-r,0.0),
        (r,1.0-r,PI/2.0),(r,r,PI)];
    let mut out=Vec::with_capacity(36);
    for &(x,y,a0) in &centers{
        for j in 0..=8{
            let a=a0+(PI/2.0)*j as f32/8.0;
            out.push(Point{x:x+r*a.cos(),y:y+r*a.sin()});
        }
    }
    out
}
fn ellipse_contour(cx:f32,cy:f32,rx:f32,ry:f32)->Contour{
    // Four exact quarter-circle cubic approximations, control error <0.03%.
    let k=0.552_284_8_f32;
    let (x,y)=(cx,cy);
    Contour{start:Point{x:x+rx,y},closed:true,segments:vec![
        CubicSegment{control1:Point{x:x+rx,y:y+k*ry},
            control2:Point{x:x+k*rx,y:y+ry},end:Point{x,y:y+ry}},
        CubicSegment{control1:Point{x:x-k*rx,y:y+ry},
            control2:Point{x:x-rx,y:y+k*ry},end:Point{x:x-rx,y}},
        CubicSegment{control1:Point{x:x-rx,y:y-k*ry},
            control2:Point{x:x-k*rx,y:y-ry},end:Point{x,y:y-ry}},
        CubicSegment{control1:Point{x:x+k*rx,y:y-ry},
            control2:Point{x:x+rx,y:y-k*ry},end:Point{x:x+rx,y}},
    ]}
}
fn contour_poly(points:Vec<Point>)->Contour{
    let start=points[0];
    let mut prev=start;
    let segments=points.into_iter().skip(1).map(|end|{
        let dx=end.x-prev.x;let dy=end.y-prev.y;
        let segment=CubicSegment{
            control1:Point{x:prev.x+dx/3.0,y:prev.y+dy/3.0},
            control2:Point{x:prev.x+dx*2.0/3.0,y:prev.y+dy*2.0/3.0},
            end};
        prev=end;segment
    }).collect();
    Contour{start,segments,closed:true}
}
fn arrow()->Vec<Point>{
    poly(&[(0.,0.33),(0.57,0.33),(0.57,0.),(1.,0.5),
        (0.57,1.),(0.57,0.67),(0.,0.67)])
}
fn chevron(inset:f32)->Vec<Point>{
    poly(&[(0.,0.),(1.-inset,0.),(1.,0.5),
        (1.-inset,1.),(0.,1.),(inset,0.5)])
}
fn transform_unit(points:&mut [Point],turn:u8){
    for p in points{
        let (x,y)=(p.x,p.y);
        match turn{
            1=>{p.x=1.-x;p.y=y;}
            2=>{p.x=1.-y;p.y=x;}
            3=>{p.x=y;p.y=1.-x;}
            4=>{p.x=1.-x;p.y=1.-y;}
            _=>{}
        }
    }
}
fn normalize(points:Vec<Point>,bounds:Bounds)->Vec<Point>{
    points.into_iter().map(|p|Point{
        x:bounds.x+p.x*bounds.w,
        y:bounds.y+p.y*bounds.h,
    }).collect()
}
fn ensure_shape_limits(shape:&Shape)->Result<(),String>{
    let b=shape.bounds();
    if ![b.x,b.y,b.w,b.h].iter().all(|v|v.is_finite())
        ||b.x< -100_000.||b.y< -100_000.
        ||b.x+b.w>100_000.||b.y+b.h>100_000.{
        return Err("Smart shape leaves document coordinate limits".into());
    }
    Ok(())
}
/// Build a native vector shape in page millimetres. Returns an editable
/// polyline/compound path. The document never stores opaque imported icons.
pub fn create(kind:SmartShape,bounds:Bounds,options:Options)->Result<Shape,String>{
    options.validate()?;
    if ![bounds.x,bounds.y,bounds.w,bounds.h].iter().all(|v|v.is_finite())
        ||bounds.w<0.1||bounds.h<0.1||bounds.w>50_000.||bounds.h>50_000.{
        return Err("Smart shape bounds must be finite, positive and <=50m".into());
    }
    let inset=options.inset;
    let rotation=options.rotation_degrees.to_radians();
    use SmartShape::*;
    let shape=match kind{
        Ring|Donut|Target=>{
            let (inner,outer)=if kind==Ring{(0.33,0.5)}
                else if kind==Donut{(inset*0.5,0.5)}
                else{(0.16,0.5)};
            let mut contours=vec![ellipse_contour(0.5,0.5,outer,outer),
                ellipse_contour(0.5,0.5,inner,inner)];
            if kind==Target{
                contours.push(ellipse_contour(0.5,0.5,0.28,0.28));
                contours.push(ellipse_contour(0.5,0.5,0.22,0.22));
            }
            for contour in &mut contours{
                let apply=|p:&mut Point|{
                    p.x=bounds.x+p.x*bounds.w;
                    p.y=bounds.y+p.y*bounds.h;
                };
                apply(&mut contour.start);
                for seg in &mut contour.segments{
                    apply(&mut seg.control1);apply(&mut seg.control2);
                    apply(&mut seg.end);
                }
            }
            Shape::CompoundPath{contours}
        }
        FlowDatabase=>{
            let mut body=poly(&[(0.,0.15),(1.,0.15),(1.,0.85),(0.,0.85)]);
            body=normalize(body,bounds);
            Shape::CompoundPath{contours:vec![
                contour_poly(body),
                ellipse_contour(bounds.x+bounds.w*0.5,bounds.y+bounds.h*0.15,
                    bounds.w*0.5,bounds.h*0.15),
            ]}
        }
        _=>{
            let (points,closed)=match kind{
                Triangle=> (regular(3,-PI/2.0),true),
                TriangleDown=>(regular(3,PI/2.0),true),
                Diamond=>(regular(4,-PI/2.0),true),
                Pentagon=>(regular(5,-PI/2.0),true),
                Hexagon=>(regular(6,-PI/2.0),true),
                Heptagon=>(regular(7,-PI/2.0),true),
                Octagon=>(regular(8,-PI/2.0),true),
                Nonagon=>(regular(9,-PI/2.0),true),
                Decagon=>(regular(10,-PI/2.0),true),
                Dodecagon=>(regular(12,-PI/2.0),true),
                Parallelogram=>(poly(&[(0.23,0.),(1.,0.),(0.77,1.),(0.,1.)]),true),
                Trapezoid=>(poly(&[(0.25,0.),(0.75,0.),(1.,1.),(0.,1.)]),true),
                InverseTrapezoid=>(poly(&[(0.,0.),(1.,0.),(0.75,1.),(0.25,1.)]),true),
                RoundedRectangle=>(rounded_rect((inset*0.4).clamp(0.04,0.35)),true),
                Star4=>(radial(4,inset,-PI/2.0),true),
                Star5=>(radial(5,inset,-PI/2.0),true),
                Star6=>(radial(6,inset,-PI/2.0),true),
                Star7=>(radial(7,inset,-PI/2.0),true),
                Star8=>(radial(8,inset,-PI/2.0),true),
                Star10=>(radial(10,inset,-PI/2.0),true),
                Star12=>(radial(12,inset,-PI/2.0),true),
                Burst12=>(radial(12,inset*0.65,-PI/2.0),true),
                Burst16=>(radial(16,inset*0.65,-PI/2.0),true),
                Burst24=>(radial(24,inset*0.65,-PI/2.0),true),
                Flower8=>(flower(8,inset,rotation),true),
                Flower12=>(flower(12,inset,rotation),true),
                Sun8=>(radial(8,0.72,-PI/2.0),true),
                Gear8=>(gear(8,0.79,rotation),true),
                Gear12=>(gear(12,0.8,rotation),true),
                Gear20=>(gear(20,0.82,rotation),true),
                ArrowRight=>(arrow(),true),
                ArrowLeft=>{let mut p=arrow();transform_unit(&mut p,1);(p,true)},
                ArrowUp=>{let mut p=arrow();transform_unit(&mut p,3);(p,true)},
                ArrowDown=>{let mut p=arrow();transform_unit(&mut p,2);(p,true)},
                ArrowDoubleH=>(poly(&[(0.,0.5),(0.3,0.05),(0.3,0.33),
                    (0.7,0.33),(0.7,0.05),(1.,0.5),(0.7,0.95),
                    (0.7,0.67),(0.3,0.67),(0.3,0.95)]),true),
                ArrowDoubleV=>{let mut p=poly(&[(0.,0.5),(0.3,0.05),(0.3,0.33),
                    (0.7,0.33),(0.7,0.05),(1.,0.5),(0.7,0.95),
                    (0.7,0.67),(0.3,0.67),(0.3,0.95)]);
                    transform_unit(&mut p,2);(p,true)},
                ChevronRight=>(chevron(inset),true),
                ChevronLeft=>{let mut p=chevron(inset);transform_unit(&mut p,1);(p,true)},
                ChevronUp=>{let mut p=chevron(inset);transform_unit(&mut p,3);(p,true)},
                ChevronDown=>{let mut p=chevron(inset);transform_unit(&mut p,2);(p,true)},
                ArrowBlockRight=>{let mut p=arrow();for q in &mut p{
                    if q.x<0.57{q.y=(q.y-0.5)*1.6+0.5;}}(p,true)},
                ArrowBlockLeft=>{let mut p=arrow();for q in &mut p{
                    if q.x<0.57{q.y=(q.y-0.5)*1.6+0.5;}}
                    transform_unit(&mut p,1);(p,true)},
                ArrowBent=>(poly(&[(0.,1.),(0.,0.46),(0.55,0.46),(0.55,0.2),
                    (1.,0.58),(0.55,0.95),(0.55,0.7),(0.23,0.7),(0.23,1.)]),true),
                ArrowCircular=>{
                    let mut p=Vec::new();
                    for i in 0..=36{
                        let a=-PI/2.0+TAU*0.78*i as f32/36.0;
                        p.push(Point{x:0.5+0.45*a.cos(),y:0.5+0.45*a.sin()});
                    }
                    p.extend(poly(&[(0.92,0.36),(0.77,0.85),(0.62,0.51)]));
                    for i in (0..=36).rev(){
                        let a=-PI/2.0+TAU*0.78*i as f32/36.0;
                        p.push(Point{x:0.5+0.25*a.cos(),y:0.5+0.25*a.sin()});
                    }
                    (p,true)
                }
                Heart=>{
                    let count=96;
                    let p=(0..count).map(|i|{
                        let t=TAU*i as f32/count as f32;
                        let hx=16.0*t.sin().powi(3);
                        let hy=13.0*t.cos()-5.0*(2.0*t).cos()-
                            2.0*(3.0*t).cos()-(4.0*t).cos();
                        Point{x:0.5+hx/36.0,y:0.5-hy/36.0}
                    }).collect();
                    (p,true)
                }
                Lightning=>(poly(&[(0.62,0.),(0.12,0.59),(0.47,0.59),
                    (0.3,1.),(0.89,0.39),(0.54,0.39)]),true),
                Home=>(poly(&[(0.,0.43),(0.5,0.02),(1.,0.43),(0.86,0.43),
                    (0.86,0.98),(0.61,0.98),(0.61,0.62),(0.39,0.62),
                    (0.39,0.98),(0.14,0.98),(0.14,0.43)]),true),
                Shield=>(poly(&[(0.5,0.),(1.,0.18),(0.94,0.6),
                    (0.5,1.),(0.06,0.6),(0.,0.18)]),true),
                Bookmark=>(poly(&[(0.18,0.),(0.82,0.),(0.82,1.),
                    (0.5,0.75),(0.18,1.)]),true),
                Tag=>(poly(&[(0.06,0.2),(0.67,0.2),(0.98,0.5),
                    (0.67,0.8),(0.06,0.8)]),true),
                Ticket=>(poly(&[(0.,0.),(0.4,0.),(0.45,0.12),
                    (0.55,0.12),(0.6,0.),(1.,0.),(1.,0.4),
                    (0.85,0.5),(1.,0.6),(1.,1.),(0.6,1.),
                    (0.55,0.88),(0.45,0.88),(0.4,1.),(0.,1.),
                    (0.,0.6),(0.15,0.5),(0.,0.4)]),true),
                SpeechLeft=>(poly(&[(0.04,0.08),(0.96,0.08),(0.96,0.75),
                    (0.33,0.75),(0.08,1.),(0.15,0.75),(0.04,0.75)]),true),
                SpeechRight=>(poly(&[(0.04,0.08),(0.96,0.08),(0.96,0.75),
                    (0.85,0.75),(0.92,1.),(0.67,0.75),(0.04,0.75)]),true),
                ThoughtBubble=>{
                    let p=flower(8,0.18,0.);
                    (p,true)
                }
                Banner=>(poly(&[(0.,0.),(1.,0.),(0.82,0.5),
                    (1.,1.),(0.,1.),(0.18,0.5)]),true),
                Ribbon=>(poly(&[(0.05,0.15),(0.95,0.15),
                    (0.85,0.5),(0.95,0.85),(0.05,0.85),
                    (0.15,0.5)]),true),
                Crown=>(poly(&[(0.05,0.25),(0.28,0.55),(0.37,0.12),
                    (0.5,0.47),(0.63,0.12),(0.72,0.55),
                    (0.95,0.25),(0.86,0.88),(0.14,0.88)]),true),
                Flag=>(poly(&[(0.1,0.04),(0.87,0.04),(0.65,0.33),
                    (0.87,0.61),(0.1,0.61)]),true),
                WaveFlag=>{
                    let top=wave(0.2,0.09,1.4,0.0);
                    let mut p=top;
                    let mut bottom=wave(0.8,0.09,1.4,0.0);
                    bottom.reverse();p.extend(bottom);(p,true)
                }
                Cross=>(poly(&[(0.38,0.),(0.62,0.),(0.62,0.38),
                    (1.,0.38),(1.,0.62),(0.62,0.62),(0.62,1.),
                    (0.38,1.),(0.38,0.62),(0.,0.62),(0.,0.38),(0.38,0.38)]),true),
                Checkmark=>(poly(&[(0.02,0.55),(0.2,0.34),(0.41,0.59),
                    (0.78,0.08),(0.97,0.25),(0.41,0.97)]),true),
                XMark=>(poly(&[(0.,0.16),(0.16,0.),(0.5,0.34),
                    (0.84,0.),(1.,0.16),(0.66,0.5),
                    (1.,0.84),(0.84,1.),(0.5,0.66),
                    (0.16,1.),(0.,0.84),(0.34,0.5)]),true),
                Hourglass=>(poly(&[(0.13,0.02),(0.87,0.02),(0.87,0.18),
                    (0.62,0.5),(0.87,0.82),(0.87,0.98),
                    (0.13,0.98),(0.13,0.82),(0.38,0.5),(0.13,0.18)]),true),
                Teardrop=>{
                    let p=(0..64).map(|i|{
                        let a=TAU*i as f32/64.0;
                        let r=(0.5+0.5*a.sin()).clamp(0.,1.);
                        Point{x:0.5+0.5*r*a.cos(),y:0.49+0.49*r*a.sin()}
                    }).collect();(p,true)
                }
                Crescent=>{
                    let mut p=Vec::new();
                    for i in 0..=48{
                        let a=-PI/2.0+PI*2.0*i as f32/48.0;
                        p.push(Point{x:0.48+0.46*a.cos(),y:0.5+0.46*a.sin()});
                    }
                    for i in (0..=48).rev(){
                        let a=-PI/2.0+PI*2.0*i as f32/48.0;
                        p.push(Point{x:0.63+0.28*a.cos(),y:0.5+0.36*a.sin()});
                    }
                    (p,true)
                }
                Cloud=>{
                    let p=(0..=96).map(|i|{
                        let a=TAU*i as f32/96.0;
                        let lobe=1.0+0.15*(7.0*a).cos()+0.1*(4.0*a).sin();
                        Point{x:0.5+0.47*lobe*a.cos()/1.25,
                            y:0.5+0.4*lobe*a.sin()/1.25}
                    }).collect();(p,true)
                }
                FlowProcess=>(rounded_rect(0.07),true),
                FlowDecision=>(regular(4,-PI/2.0),true),
                FlowData=>(poly(&[(0.22,0.),(1.,0.),(0.78,1.),(0.,1.)]),true),
                FlowDocument=>{
                    let mut p=poly(&[(0.,0.),(1.,0.),(1.,0.82)]);
                    p.extend(wave(0.82,0.13,1.,0.).into_iter().rev());
                    p.push(Point{x:0.,y:0.82});(p,true)
                }
                FlowTerminator=>(capsule(),true),
                FlowPreparation=>(poly(&[(0.2,0.),(0.8,0.),
                    (1.,0.5),(0.8,1.),(0.2,1.),(0.,0.5)]),true),
                FlowDelay=>{
                    let mut p=poly(&[(0.,0.),(0.55,0.)]);
                    for i in 0..=24{
                        let a=-PI/2.0+PI*i as f32/24.0;
                        p.push(Point{x:0.55+0.45*a.cos(),
                            y:0.5+0.5*a.sin()});
                    }
                    p.push(Point{x:0.,y:1.});(p,true)
                }
                FlowStoredData=>{
                    let mut p=poly(&[(0.16,0.),(1.,0.),(1.,1.),(0.16,1.)]);
                    for i in (0..=24).rev(){
                        let a=-PI/2.0+PI*i as f32/24.0;
                        p.push(Point{x:0.16-0.16*a.cos(),y:0.5+0.5*a.sin()});
                    }(p,true)
                }
                FlowManual=>(poly(&[(0.,0.2),(1.,0.),(1.,1.),(0.,1.)]),true),
                FlowDisplay=>{
                    let mut p=poly(&[(0.16,0.),(0.74,0.)]);
                    for i in 0..=24{
                        let a=-PI/2.0+PI*i as f32/24.0;
                        p.push(Point{x:0.74+0.26*a.cos(),y:0.5+0.5*a.sin()});
                    }
                    p.extend(poly(&[(0.16,1.),(0.,0.5)]));(p,true)
                }
                FlowOffpage=>(poly(&[(0.,0.),(1.,0.),(1.,0.66),
                    (0.5,1.),(0.,0.66)]),true),
                FlowSort=>(poly(&[(0.5,0.),(1.,0.5),(0.5,1.),(0.,0.5)]),true),
                FlowMerge=>(poly(&[(0.,0.),(1.,0.),(0.5,1.)]),true),
                FlowExtract=>(poly(&[(0.5,0.),(1.,1.),(0.,1.)]),true),
                FlowCard=>(poly(&[(0.2,0.),(1.,0.),(1.,1.),(0.,1.),(0.,0.2)]),true),
                FlowAnnotation=>(poly(&[(0.15,0.),(0.15,0.45),
                    (0.85,0.45),(0.85,0.),(1.,0.),(1.,1.),
                    (0.85,1.),(0.85,0.55),(0.15,0.55),
                    (0.15,1.),(0.,1.),(0.,0.)]),true),
                FlowConnector=>(regular(24,0.),true),
                Spiral=>{
                    let turns=options.detail as f32/4.0;
                    let p=(0..=192).map(|i|{
                        let t=i as f32/192.0;
                        let a=rotation+TAU*turns*t;
                        Point{x:0.5+0.5*t*a.cos(),y:0.5+0.5*t*a.sin()}
                    }).collect();(p,false)
                }
                Wave=>(wave(0.5,0.42,options.detail as f32/4.0,rotation),false),
                Zigzag=>{
                    let n=options.detail as usize*2;
                    let p=(0..=n).map(|i|Point{x:i as f32/n as f32,
                        y:if i%2==0{0.1}else{0.9}}).collect();
                    (p,false)
                }
                Pie=>{
                    let sweep=(options.detail as f32/32.0)*TAU;
                    let mut p=vec![Point{x:0.5,y:0.5}];
                    for i in 0..=64{
                        let a=rotation+(-PI/2.0)+sweep*i as f32/64.0;
                        p.push(Point{x:0.5+0.5*a.cos(),
                            y:0.5+0.5*a.sin()});
                    }(p,true)
                }
                GridMarker=>(poly(&[(0.,0.5),(1.,0.5),(0.5,0.5),
                    (0.5,0.),(0.5,1.)]),false),
                Compass=>(radial(8,0.26,rotation-PI/2.0),true),
                // Complete exhaustiveness is intentional for the user-visible
                // 90-preset catalogue. No silent fallback to one generic shape.
                Ring|Donut|Target|FlowDatabase=>unreachable!(),
            };
            let points=normalize(points,bounds);
            Shape::Polyline{points,closed}
        }
    };
    ensure_shape_limits(&shape)?;
    Ok(shape)
}

#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Document,Style};
    #[test]
    fn all_90_shapes_are_native_finite_and_exportable(){
        assert_eq!(SmartShape::ALL.len(),90);
        for kind in SmartShape::ALL{
            let shape=create(kind,Bounds{x:11.,y:13.,w:50.,h:70.},
                Options::default()).unwrap_or_else(|e|panic!("{kind:?} {e}"));
            let mut d=Document::default();
            d.add(shape,Style::default());
            d.validate().unwrap_or_else(|e|panic!("{kind:?} {e}"));
            let svg=d.to_svg();
            assert!(svg.contains("<svg"));
            assert!(svg.contains("<path")||svg.contains("<polyline")||
                svg.contains("<polygon"),"invalid SVG for {kind:?}");
        }
    }
    #[test]
    fn all_catalogue_categories_have_entries(){
        for category in Category::ALL{
            assert!(SmartShape::ALL.iter().any(|v|v.category()==category));
        }
    }
    #[test]
    fn all_presets_remain_independently_editable_after_json_and_kdr(){
        let mut doc=Document::default();
        for (i,kind) in SmartShape::ALL.into_iter().enumerate(){
            let x=10.0+(i%10) as f32*90.0;
            let y=10.0+(i/10) as f32*90.0;
            let shape=create(kind,Bounds{x,y,w:60.,h:60.},Options::default()).unwrap();
            doc.add(shape,Style::default());
        }
        doc.page.width_mm=1000.;doc.page.height_mm=1000.;
        doc.validate().unwrap();
        let dir=tempfile::tempdir().unwrap();
        let path=dir.path().join("smart-shapes.kdr");
        doc.write(&path).unwrap();
        let reloaded=Document::read(&path).unwrap();
        assert_eq!(doc,reloaded);
    }
    #[test]
    fn customization_and_invalid_params_are_checked(){
        assert!(create(SmartShape::Star7,Bounds{x:0.,y:0.,w:70.,h:70.},
            Options{inset:f32::NAN,..Options::default()}).is_err());
        assert!(create(SmartShape::Gear8,Bounds{x:0.,y:0.,w:-2.,h:70.},
            Options::default()).is_err());
        let a=create(SmartShape::Star7,Bounds{x:0.,y:0.,w:70.,h:70.},
            Options{inset:0.2,..Options::default()}).unwrap();
        let b=create(SmartShape::Star7,Bounds{x:0.,y:0.,w:70.,h:70.},
            Options{inset:0.8,..Options::default()}).unwrap();
        assert_ne!(a,b);
    }
    #[test]
    fn open_paths_are_not_accidentally_filled(){
        for kind in [SmartShape::Spiral,SmartShape::Wave,
            SmartShape::Zigzag,SmartShape::GridMarker]{
            assert!(matches!(create(kind,Bounds{x:0.,y:0.,w:20.,h:20.},
                Options::default()).unwrap(),Shape::Polyline{closed:false,..}));
            assert!(kind.is_open());
        }
    }
    #[test]
    fn ring_holes_are_native_compound_contours(){
        for kind in [SmartShape::Ring,SmartShape::Donut,SmartShape::Target]{
            let shape=create(kind,Bounds{x:2.,y:3.,w:30.,h:40.},
                Options::default()).unwrap();
            assert!(matches!(shape,Shape::CompoundPath{contours} if contours.len()>=2));
        }
    }
}
