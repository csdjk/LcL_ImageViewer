//! Direct COM thumbnail probe; never registers a DLL or changes file associations.
//! Args: <iv_shell.dll> <new output directory> <input file>...
//! Fails rather than skipping missing inputs or failed thumbnail calls.
use std::path::{Path, PathBuf};
use windows::core::{Interface, GUID, HSTRING};
use windows::Win32::Foundation::{BOOL, HMODULE};
use windows::Win32::Graphics::Gdi::{DeleteObject, GetObjectW, BITMAP, HBITMAP, HGDIOBJ};
use windows::Win32::System::Com::{CoInitializeEx, IClassFactory, IStream, COINIT_APARTMENTTHREADED, STGM_READ};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::PropertiesSystem::IInitializeWithStream;
use windows::Win32::UI::Shell::{IThumbnailProvider, SHCreateStreamOnFileEx, WTS_ALPHATYPE};

const CLSID: GUID = GUID::from_u128(0x7a3e9b21_4c5d_4e8f_9a6b_1d2c3e4f5a6b);
type GetClass = unsafe extern "system" fn(*const GUID,*const GUID,*mut *mut core::ffi::c_void)->i32;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    if args.len()<3 { return Err("usage: test_thumbnail <dll> <new output dir> <input>...".into()); }
    assert!(args[0].is_file());
    std::fs::create_dir(&args[1])?;
    unsafe { CoInitializeEx(None,COINIT_APARTMENTTHREADED).ok()?; }
    let library: HMODULE=unsafe {LoadLibraryW(&HSTRING::from(args[0].as_os_str()))?};
    let symbol=unsafe {GetProcAddress(library,windows::core::s!("DllGetClassObject"))}.ok_or("missing class factory")?;
    let get: GetClass=unsafe {std::mem::transmute(symbol)};
    let mut ptr=std::ptr::null_mut();
    let hr=unsafe {get(&CLSID,&IClassFactory::IID,&mut ptr)};
    assert_eq!(hr,0,"class factory failed");
    let factory=unsafe {IClassFactory::from_raw(ptr)};
    for input in &args[2..] {
        assert!(input.is_file(),"input missing: {}",input.display());
        for size in [128,512] { probe(&factory,input,&args[1],size)?; }
    }
    // Interfaces are released before process exit; no registry activation or Explorer restart.
    Ok(())
}

fn probe(factory:&IClassFactory,input:&Path,out:&Path,cx:u32)->Result<(),Box<dyn std::error::Error>> {
    let provider:IThumbnailProvider=unsafe {factory.CreateInstance(None)?};
    let init:IInitializeWithStream=provider.cast()?;
    let stream:IStream=unsafe {SHCreateStreamOnFileEx(&HSTRING::from(input.as_os_str()),STGM_READ.0,0,BOOL(0),None)?};
    unsafe {init.Initialize(&stream,0)?;}
    let mut bitmap=HBITMAP::default();
    let mut alpha=WTS_ALPHATYPE(0);
    unsafe {provider.GetThumbnail(cx,&mut bitmap,&mut alpha)?;}
    let result=(||->Result<(),Box<dyn std::error::Error>>{
        let mut bm=BITMAP::default();
        let got=unsafe {GetObjectW(HGDIOBJ(bitmap.0),std::mem::size_of::<BITMAP>() as i32,Some(&mut bm as *mut _ as *mut _))};
        assert_ne!(got,0); assert!(!bm.bmBits.is_null());
        assert!(bm.bmWidth>0 && bm.bmHeight>0 && bm.bmBitsPixel==32);
        assert!(bm.bmWidth as u32<=cx && bm.bmHeight as u32<=cx);
        let raw=unsafe {std::slice::from_raw_parts(bm.bmBits as *const u8,(bm.bmWidthBytes*bm.bmHeight) as usize)};
        assert!(raw.chunks_exact(4).any(|p|p[3]>0),"all transparent preview");
        assert!(raw.chunks_exact(4).all(|p|p[..3].iter().all(|v|*v<=p[3])),"not premultiplied alpha");
        let stem=format!("{}-{cx}",input.file_name().unwrap().to_string_lossy());
        std::fs::write(out.join(format!("{stem}.bgra")),raw)?;
        std::fs::write(out.join(format!("{stem}.json")),format!("{{\"width\":{},\"height\":{},\"stride\":{},\"alpha\":{},\"cx\":{cx}}}",bm.bmWidth,bm.bmHeight,bm.bmWidthBytes,alpha.0))?;
        println!("PASS\t{stem}\t{}x{}\talpha={}",bm.bmWidth,bm.bmHeight,alpha.0);
        Ok(())
    })();
    let _=unsafe {DeleteObject(HGDIOBJ(bitmap.0))};
    result
}
