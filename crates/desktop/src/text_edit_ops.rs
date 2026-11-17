//! Unicode-safe, bounded text editing operations for vector text objects.
//! All offsets use Rust UTF-8 string operations and never split code points.

pub const MAX_EDITED_TEXT_BYTES:usize=256*1024;

/// Literal (not regex) global replacement. Reject an empty needle, which would
/// otherwise match between every UTF-8 code point, and bound the output size.
pub fn replace_all_literal(value:&str,find:&str,replacement:&str)->Result<(String,usize),String>{
    if find.is_empty(){return Err("Search text must not be empty".into());}
    if value.len()>MAX_EDITED_TEXT_BYTES{
        return Err("Text object exceeds the supported editing limit".into());
    }
    let count=value.matches(find).count();
    if count==0{return Ok((value.to_owned(),0));}
    let removed=count.checked_mul(find.len())
        .ok_or("Replacement length overflow")?;
    let added=count.checked_mul(replacement.len())
        .ok_or("Replacement length overflow")?;
    let target=value.len().checked_sub(removed)
        .and_then(|n|n.checked_add(added))
        .ok_or("Replacement length overflow")?;
    if target>MAX_EDITED_TEXT_BYTES{
        return Err("Replacement would exceed the maximum text size".into());
    }
    Ok((value.replace(find,replacement),count))
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn replaces_cyrillic_and_preserves_other_characters(){
        let (out,count)=replace_all_literal("Привет, мир! мир", "мир", "друг").unwrap();
        assert_eq!(count,2);
        assert_eq!(out,"Привет, друг! друг");
    }
    #[test]
    fn matches_are_literal_not_regular_expressions(){
        let (out,count)=replace_all_literal("a.b a*b", "a.b", "x").unwrap();
        assert_eq!((out,count),("x a*b".into(),1));
    }
    #[test]
    fn rejects_empty_search_and_output_explosion(){
        assert!(replace_all_literal("abc","","x").is_err());
        assert!(replace_all_literal("a","a",&"x".repeat(MAX_EDITED_TEXT_BYTES+1)).is_err());
    }
    #[test]
    fn missing_match_does_not_mutate(){
        assert_eq!(replace_all_literal("Ёжик","нет","да").unwrap(),
            ("Ёжик".into(),0));
    }
    #[test]
    fn embedded_null_and_multibyte_do_not_break_boundaries(){
        let (out,count)=replace_all_literal("🌍\0🌍","🌍","☀️").unwrap();
        assert_eq!(count,2);
        assert_eq!(out,"☀️\0☀️");
    }
}
