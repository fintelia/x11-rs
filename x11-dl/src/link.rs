// x11-rs: Rust bindings for X11 libraries
// The X11 libraries are available under the MIT license.
// These bindings are public domain.

use std::ffi::{ CStr, CString };
use std::path::Path;
use std::os::raw::{ c_char, c_void };

use libc;

use error::{ OpenError, OpenErrorKind };

include!(concat!(env!("OUT_DIR"), "/config.rs"));

//
// x11_link!
//

macro_rules! x11_link {
  { $struct_name:ident, $pkg_name:ident, [$($lib_name:expr),*], $nsyms:expr,
    $(pub fn $fn_name:ident ($($param_name:ident : $param_type:ty),*) -> $ret_type:ty,)*
    variadic:
    $(pub fn $vfn_name:ident ($($vparam_name: ident : $vparam_type:ty),+) -> $vret_type:ty,)*
    globals:
    $(pub static $var_name:ident : $var_type:ty,)*
  } => {
    pub struct $struct_name {
      #[allow(dead_code)]
      lib: ::link::DynamicLibrary,
      $(pub $fn_name: unsafe extern "C" fn ($($param_type),*) -> $ret_type,)*
      $(pub $vfn_name: unsafe extern "C" fn ($($vparam_type),+, ...) -> $vret_type,)*
      $(pub $var_name: *mut $var_type,)*
    }

    unsafe impl Send for $struct_name {}
    unsafe impl Sync for $struct_name {}

    impl $struct_name {
      pub fn open () -> Result<$struct_name, $crate::error::OpenError> {
        let libdir = $crate::link::config::libdir::$pkg_name;
        let lib = try!($crate::link::DynamicLibrary::open_multi(libdir, &[$($lib_name),*]));
        unsafe {
          Ok(Self {
              $($fn_name: ::std::mem::transmute(try!(lib.symbol(stringify!($fn_name)))),)*
              $($vfn_name: ::std::mem::transmute(try!(lib.symbol(stringify!($vfn_name)))),)*
              $($var_name: try!(lib.symbol(stringify!($var_name))) as *mut $var_type,)*
              lib,
          })
        }
      }
    }
  };
}


//
// DynamicLibrary
//


pub struct DynamicLibrary {
  handle: *mut c_void,
}

impl DynamicLibrary {
  pub fn open (name: &str) -> Result<DynamicLibrary, OpenError> {
    unsafe {
      let cname = match CString::new(name) {
        Ok(cname) => cname,
        Err(_) => {
          return Err(OpenError::new(OpenErrorKind::Library,
                                    String::from("library name contains NUL byte(s)")));
        },
      };

      let handle = libc::dlopen(cname.as_ptr(), libc::RTLD_LAZY);

      if handle.is_null() {
        let msg = libc::dlerror();

        if msg.is_null() {
          return Err(OpenError::new(OpenErrorKind::Library, String::new()));
        }

        let cmsg = CStr::from_ptr(msg as *const c_char);
        let detail = cmsg.to_string_lossy().into_owned();
        return Err(OpenError::new(OpenErrorKind::Library, detail));
      }

      Ok(DynamicLibrary { handle: handle as *mut c_void })
    }
  }

  pub fn open_multi (libdir: Option<&'static str>, names: &[&str]) -> Result<DynamicLibrary, OpenError> {
    assert!(!names.is_empty());

    let paths = libdir.map_or(Vec::new(), |dir| {
        let path = Path::new(dir);
        names.iter().map(|name| path.join(name).to_str().unwrap().to_string()).collect::<Vec<_>>()
    });

    let mut msgs = Vec::new();

    for name in names.iter().map(|x| *x).chain(paths.iter().map(|x| &**x)) {
      match DynamicLibrary::open(name) {
        Ok(lib) => { return Ok(lib); },
        Err(err) => { msgs.push(format!("{}", err)); },
      }
    }

    let mut detail = String::new();

    for i in 0..msgs.len() {
      if i != 0 {
        detail.push_str("; ");
      }
      detail.push_str(msgs[i].as_ref());
    }

    Err(OpenError::new(OpenErrorKind::Library, detail))
  }

  pub fn symbol (&self, name: &str) -> Result<*mut c_void, OpenError> {
    unsafe {
      let cname = match CString::new(name) {
        Ok(cname) => cname,
        Err(_) => {
          return Err(OpenError::new(OpenErrorKind::Symbol,
                                    String::from("symbol name contains NUL byte(s)")));
        },
      };

      let sym = libc::dlsym(self.handle as *mut _, cname.as_ptr());

      if sym.is_null() {
        let msg = libc::dlerror();

        if msg.is_null() {
          return Err(OpenError::new(OpenErrorKind::Symbol, String::from(name)));
        }

        let cmsg = CStr::from_ptr(msg as *const c_char);
        let detail = format!("{} - {}", name, cmsg.to_string_lossy().into_owned());;
        return Err(OpenError::new(OpenErrorKind::Symbol, detail));
      }

      Ok(sym as *mut c_void)
    }
  }
}

impl Drop for DynamicLibrary {
  fn drop (&mut self) {
    unsafe {
      libc::dlclose(self.handle as *mut _);
    }
  }
}
