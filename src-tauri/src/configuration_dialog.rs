//! Native folder selection; the dashboard receives only the selected path.
pub fn choose(owner: isize) -> Result<Option<String>, String> {
    #[cfg(windows)]
    {
        use windows_bindings::{
            core::w,
            Win32::{
                Foundation::HWND,
                System::Com::{
                    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize,
                    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
                },
                UI::Shell::{
                    FileOpenDialog, IFileOpenDialog, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST,
                    FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
                },
            },
        };
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe { CoUninitialize() };
            }
        }
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                .ok()
                .map_err(|_| "storage-folder-dialog")?;
            let _apartment = Apartment;
            let dialog: IFileOpenDialog =
                CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)
                    .map_err(|_| "storage-folder-dialog")?;
            let options = dialog.GetOptions().map_err(|_| "storage-folder-dialog")?;
            dialog
                .SetOptions(options | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST | FOS_PICKFOLDERS)
                .map_err(|_| "storage-folder-dialog")?;
            dialog
                .SetTitle(w!("Choose configuration folder"))
                .map_err(|_| "storage-folder-dialog")?;
            if let Err(error) = dialog.Show(Some(HWND(owner as *mut _))) {
                if error.code().0 as u32 == 0x800704c7 {
                    return Ok(None);
                }
                return Err("storage-folder-dialog".into());
            }
            let item = dialog.GetResult().map_err(|_| "storage-folder-dialog")?;
            let value = item
                .GetDisplayName(SIGDN_FILESYSPATH)
                .map_err(|_| "storage-folder-dialog")?;
            let path = value.to_string().map_err(|_| "storage-folder-dialog");
            CoTaskMemFree(Some(value.as_ptr().cast()));
            Ok(Some(path?))
        }
    }
    #[cfg(not(windows))]
    {
        let _ = owner;
        Err("storage-folder-dialog".into())
    }
}
