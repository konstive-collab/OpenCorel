//! Versioned Kodi Draw document (.kdr). This is a ZIP container, not a CorelDRAW CDR.
use crate::Document;
use serde::{Serialize,Deserialize};
use sha2::{Digest,Sha256};
use std::{collections::HashSet,fs::{self,File},io::{Read,Write},path::Path};
use zip::{ZipArchive,ZipWriter,write::SimpleFileOptions,CompressionMethod};

pub const KDR_VERSION:u32=1;
const MAX_ARCHIVE_SIZE:u64=128*1024*1024;
const MAX_DOCUMENT_SIZE:u64=96*1024*1024;
const MAX_PREVIEW_SIZE:u64=30*1024*1024;
const MAX_MANIFEST_SIZE:u64=16*1024;

#[derive(Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest{
    format:String,
    version:u32,
    document:String,
    sha256:String,
    generator:String,
}
fn checksum(bytes:&[u8])->String {format!("{:x}",Sha256::digest(bytes))}
fn io_error(err:impl std::fmt::Display)->String{err.to_string()}

pub fn save_kdr(doc:&Document,path:impl AsRef<Path>)->Result<(),String>{
    doc.validate()?;
    let raw=serde_json::to_vec(doc).map_err(io_error)?;
    if raw.len() as u64>MAX_DOCUMENT_SIZE{
        return Err("KDR project exceeds the supported document size".into());
    }
    let manifest=Manifest{
        format:"kodi-draw".into(),version:KDR_VERSION,
        document:"document.json".into(),sha256:checksum(&raw),
        generator:"OpenCorel".into()
    };
    let metadata=serde_json::to_vec(&manifest).map_err(io_error)?;
    let preview=doc.to_svg();
    if preview.len() as u64>MAX_PREVIEW_SIZE{
        return Err("KDR preview exceeds the supported size".into());
    }
    let path=path.as_ref();
    let dir=path.parent().filter(|p|!p.as_os_str().is_empty())
        .unwrap_or_else(||Path::new("."));
    let mut temp=tempfile::Builder::new().prefix(".kdr-write-")
        .tempfile_in(dir).map_err(io_error)?;
    {
        let mut zip=ZipWriter::new(temp.as_file_mut());
        let options=SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        zip.start_file("manifest.json",options).map_err(io_error)?;
        zip.write_all(&metadata).map_err(io_error)?;
        zip.start_file("document.json",options).map_err(io_error)?;
        zip.write_all(&raw).map_err(io_error)?;
        zip.start_file("preview.svg",options).map_err(io_error)?;
        zip.write_all(preview.as_bytes()).map_err(io_error)?;
        zip.finish().map_err(io_error)?;
    }
    temp.as_file().sync_all().map_err(io_error)?;
    temp.persist(path).map_err(|e|e.error.to_string())?;
    Ok(())
}

pub fn read_kdr(path:impl AsRef<Path>)->Result<Document,String>{
    let path=path.as_ref();
    let size=fs::metadata(path).map_err(io_error)?.len();
    if size>MAX_ARCHIVE_SIZE {return Err("KDR file is too large".into());}
    let handle=File::open(path).map_err(io_error)?;
    let mut archive=ZipArchive::new(handle).map_err(|e|format!("Invalid KDR ZIP: {e}"))?;
    if archive.len()<2 || archive.len()>10{return Err("Unexpected KDR container entries".into());}
    let mut seen=HashSet::new();
    for i in 0..archive.len(){
        let entry=archive.by_index(i).map_err(io_error)?;
        let name=entry.name().to_string();
        if !seen.insert(name.clone()){
            return Err("Duplicate file name inside KDR container".into());
        }
        let limit=match name.as_str(){
            "manifest.json"=>MAX_MANIFEST_SIZE,
            "document.json"=>MAX_DOCUMENT_SIZE,
            "preview.svg"=>MAX_PREVIEW_SIZE,
            _=>return Err(format!("Unsupported KDR archive entry: {name}")),
        };
        if entry.size()>limit{return Err(format!("KDR entry exceeds limit: {name}"));}
    }
    if !seen.contains("manifest.json") || !seen.contains("document.json"){
        return Err("KDR is missing its manifest or document".into());
    }
    let read_entry=|name:&str,maximum:u64,archive:&mut ZipArchive<File>|->Result<Vec<u8>,String>{
        let mut item=archive.by_name(name).map_err(io_error)?;
        let mut bytes=Vec::new();
        item.take(maximum+1).read_to_end(&mut bytes).map_err(io_error)?;
        if bytes.len() as u64>maximum{return Err("Expanded KDR entry exceeds size limit".into());}
        Ok(bytes)
    };
    let meta_bytes=read_entry("manifest.json",MAX_MANIFEST_SIZE,&mut archive)?;
    let meta:Manifest=serde_json::from_slice(&meta_bytes).map_err(io_error)?;
    if meta.format!="kodi-draw" || meta.version!=KDR_VERSION ||
        meta.document!="document.json"{
        return Err(format!("Unsupported KDR format/version: {} v{}",meta.format,meta.version));
    }
    let bytes=read_entry("document.json",MAX_DOCUMENT_SIZE,&mut archive)?;
    if checksum(&bytes)!=meta.sha256{
        return Err("KDR integrity check failed: project data differs from manifest".into());
    }
    let doc:Document=serde_json::from_slice(&bytes).map_err(io_error)?;
    doc.validate()?;
    Ok(doc)
}
#[cfg(test)]
mod tests{
    use super::*;
    use crate::{Shape,Style};
    #[test]
    fn kdr_roundtrip_with_objects_and_layers(){
        let dir=tempfile::tempdir().unwrap();
        let mut doc=Document::default();
        let layer=doc.add_layer("Карта".into());
        doc.add_to_layer(Shape::Rectangle{x:5.0,y:6.0,w:20.0,h:12.0},
            Style::default(),layer);
        let file=dir.path().join("test.kdr");
        save_kdr(&doc,&file).unwrap();
        assert_eq!(read_kdr(&file).unwrap(),doc);
        let bytes=fs::read(&file).unwrap();
        assert!(bytes.starts_with(b"PK"));
        assert!(bytes.len()<40_000);
    }
    #[test]
    fn damaged_document_checksum_is_rejected(){
        let dir=tempfile::tempdir().unwrap();
        let file=dir.path().join("bad.kdr");
        let mut handle=File::create(&file).unwrap();
        {
            let mut writer=ZipWriter::new(&mut handle);
            let options=SimpleFileOptions::default();
            writer.start_file("manifest.json",options).unwrap();
            writer.write_all(br#"{"format":"kodi-draw","version":1,"document":"document.json","sha256":"bad","generator":"test"}"#).unwrap();
            writer.start_file("document.json",options).unwrap();
            writer.write_all(br#"{}"#).unwrap();
            writer.finish().unwrap();
        }
        assert!(read_kdr(&file).unwrap_err().contains("integrity"));
    }
    #[test]
    fn invalid_save_preserves_previous_project(){
        let dir=tempfile::tempdir().unwrap();
        let file=dir.path().join("good.kdr");
        let good=Document::default();save_kdr(&good,&file).unwrap();
        let mut invalid=good.clone();invalid.page.width_mm=f32::NAN;
        assert!(save_kdr(&invalid,&file).is_err());
        assert_eq!(read_kdr(&file).unwrap(),good);
    }
}
