use openvr_sys;
use openvr_sys::EVRRenderModelError::*;

use std::string::String;
use std::ptr::null_mut;
use std::slice;
use subsystems::render_models;
use error::*;

pub struct IVRRenderModels(pub *const ());

pub struct RenderModel(*mut openvr_sys::RenderModel_t);
pub struct RenderModelTexture(*mut openvr_sys::RenderModel_TextureMap_t);

trait AsyncError {
    /// checks if result is currently loading
    fn is_loading(&self) -> bool;
}

impl AsyncError for Error<openvr_sys::EVRRenderModelError> {
    fn is_loading(&self) -> bool {
        match self.to_raw() {
            EVRRenderModelError_VRRenderModelError_Loading => {
                true
            },
            _ => {
                false
            }
        }
    }
}

impl Drop for RenderModel {
    /// will inform openvr that the memory for the render model is no longer required
    fn drop (&mut self) {
        unsafe {
            let models = * { render_models().unwrap().0 as *mut openvr_sys::VR_IVRRenderModels_FnTable};
            models.FreeRenderModel.unwrap()(
                self.0
            );
        }
    }
}

impl Drop for RenderModelTexture {
    /// will inform openvr that the memory for the render model is no longer required
    fn drop (&mut self) {
        unsafe {
            let models = * { render_models().unwrap().0 as *mut openvr_sys::VR_IVRRenderModels_FnTable};
            models.FreeTexture.unwrap()(
                self.0
            );
        }
    }
}

impl RenderModel {
    /// Returns an iterator that iterates over vertices
    pub fn vertex_iter(&self) -> slice::Iter<openvr_sys::RenderModel_Vertex_t> {
        unsafe {
            let slice = slice::from_raw_parts((*self.0).rVertexData, (*self.0).unVertexCount as usize);
            slice.iter()
        }
    }

    /// Returns an iterator that iterates over indices
    pub fn index_iter(&self) -> slice::Iter<u16> {
        unsafe {
            let slice = slice::from_raw_parts((*self.0).rIndexData, (*self.0).unTriangleCount as usize * 3);
            slice.iter()
        }
    }

    /// asynchronosly loads the texture for the current render model
    /// see IVRRenderModels::load_async for info how openvr async work
    pub fn load_texture_async(&self) -> Result<RenderModelTexture, Error<openvr_sys::EVRRenderModelError>> {
        unsafe {
            let models = * { render_models().unwrap().0 as *mut openvr_sys::VR_IVRRenderModels_FnTable};
            let mut resp: *mut openvr_sys::RenderModel_TextureMap_t = null_mut();

            let err = models.LoadTexture_Async.unwrap()(
                (*self.0).diffuseTextureId,
                &mut resp
            );

            match err {
                EVRRenderModelError_VRRenderModelError_None => {
                    Ok(RenderModelTexture (resp))
                },
                _ => {
                    Err(Error::from_raw(err))
                }
            }

        }
    }

    /// loads the texture for current model
    pub fn load_texture(&self) -> Result<RenderModelTexture, Error<openvr_sys::EVRRenderModelError>> {
        use std;

        loop {
            let result = self.load_texture_async();
            match result {
                Ok(texture) => {
                    return Ok(texture);
                },
                Err(err) => {
                    if !err.is_loading() {
                        return Err(err);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

impl RenderModelTexture {
    /// Returns the dimension from the texture (width, height)
    pub fn dimension(&self) -> (usize, usize) {
        unsafe {
            ((*self.0).unWidth as usize, (*self.0).unHeight as usize)
        }
    }

    /// Creates a 1 dimensional vector of pixels, format: rgba@32
    pub fn to_vec(&self) -> Vec<u8> {
        unsafe {
            let dimension = self.dimension();
            let slice = slice::from_raw_parts((*self.0).rubTextureMapData, dimension.0 * dimension.1 * 4);
            let mut vec = Vec::new();
            vec.extend_from_slice(slice);
            vec
        }
    }
}

impl IVRRenderModels {
    pub unsafe fn from_raw(ptr: *const ()) -> Self {
        IVRRenderModels(ptr as *mut ())
    }

    /// Returns the amount of render models available
    pub fn get_count(&self) -> u32 {
        unsafe {
            let models = * { self.0 as *mut openvr_sys::VR_IVRRenderModels_FnTable};

            models.GetRenderModelCount.unwrap()()
        }
    }

    /// Returns the name of an available render model
    pub fn get_name(&self, index: u32) -> String {
        unsafe {
            let models = *{ render_models.0 as *mut openvr_sys::VR_IVRRenderModels_FnTable };
            let get_name_function = models.GetRenderModelName.unwrap();
            let mut empty = vec![0i8;0];
            let required = get_name_function(index, empty.as_mut_ptr(), 0);
            if required == 0 {
                return String::from("");
            }
            let mut name: Vec<u8> = Vec::with_capacity(required as usize);
            let size = get_name_function(index, name.as_mut_ptr() as *mut i8, required);
            if (size != required) {
                panic!("name size changed");
            }
            let size_without_terminator = size - 1;
            name.set_len(size_without_terminator as usize);
            if let Ok(string) = CString::from_vec_unchecked(name).into_string() {
                return string;
            } else {
                panic!("name cannot convert to utf8");
            }
        };
    }

    /// Loads an render model into local memory
    ///  blocks the thread and waits until driver responds with model
    pub fn load(&self, name: String) -> Result<RenderModel, Error<openvr_sys::EVRRenderModelError>> {
        use std;

        loop {
            let result = self.load_async(name.clone());
            match result {
                Ok(model) => {
                    return Ok(model);
                },
                Err(err) => {
                    if !err.is_loading() {
                        return Err(err);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }

    /// Loads an render model into local memory
    ///  When called for the first time openvr will start to load the model into memory
    ///  In the mean time this call will respond with EVRRenderModelError_VRRenderModelError_Loading
    ///  It is designed to be used wihtin the render loop as it won't block the user, for sync usage use load()
    pub fn load_async(&self, name: String) -> Result<RenderModel, Error<openvr_sys::EVRRenderModelError>> {
        use std;

        unsafe {
            let models = * { self.0 as *mut openvr_sys::VR_IVRRenderModels_FnTable};
            let mut resp: *mut openvr_sys::RenderModel_t = null_mut();
            let cname = std::ffi::CString::new(name.as_str()).unwrap();
            let rawname = cname.into_raw();

            let err = models.LoadRenderModel_Async.unwrap()(
                rawname,
                &mut resp
            );

            let _ = std::ffi::CString::from_raw(rawname);

            match err {
                EVRRenderModelError_VRRenderModelError_None => {
                    Ok(RenderModel ( resp ))
                },
                _ => {
                    Err(Error::from_raw(err))
                }
            }
        }
    }
}
