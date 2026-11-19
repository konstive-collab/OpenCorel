//! On-demand asset inventory UI. Heavy full-publication traversal runs only
//! when the user requests it, never on each egui paint frame.
use super::Editor;
impl Editor{
    pub(super) fn refresh_inventory(&mut self){
        let inventory=self.doc.inventory();
        let mut lines=vec![
            format!("Страниц: {} · объектов: {}",inventory.pages,inventory.objects),
            format!("Редактируемых векторов: {}",inventory.editable_vectors),
            format!("Редактируемых надписей: {}",inventory.editable_text),
            format!("Градиентов: {} · элементов групп: {}",
                inventory.gradients,inventory.grouped_members),
            format!("Целей обтравки: {}",inventory.clipped_targets),
            format!("Встроенных SVG: {} · изображений: {}",
                inventory.embedded_svg,inventory.raster_images),
            format!("Приблизительный объём растров: {:.2} МиБ",
                inventory.estimated_image_bytes as f64/1_048_576.),
            "Шрифты:".into(),
        ];
        for (name,count) in inventory.font_families{
            lines.push(format!("  {name}: {count}"));
        }
        lines.push("Используемые цвета RGB:".into());
        for (color,count) in inventory.solid_colors{
            lines.push(format!("  {color}: {count}"));
        }
        self.status="Паспорт макета составлен по всем страницам".into();
        self.inventory_lines=lines;
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test] fn opening_inventory_does_not_change_kdr(){
        let mut editor=Editor::default();
        let before=editor.doc.clone();
        editor.refresh_inventory();
        assert_eq!(editor.doc,before);
        assert!(!editor.inventory_lines.is_empty());
    }
}
