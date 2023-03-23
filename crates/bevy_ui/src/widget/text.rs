use std::sync::Arc;

use crate::{CalculatedSize, Measure, Node, UiScale};
use bevy_asset::Assets;
use bevy_ecs::{
    entity::Entity,
    query::{Changed, Or, With},
    system::{Local, ParamSet, Query, Res, ResMut},
};
use bevy_math::Vec2;
use bevy_render::texture::Image;
use bevy_sprite::TextureAtlas;
use bevy_text::{
    TextShaper, Font, FontAtlasSet, FontAtlasWarning, Text, TextError, TextLayoutInfo,
    TextPipeline, TextSettings, YAxisOrientation, AutoTextInfo,
};
use bevy_utils::tracing::level_filters;
use bevy_window::{PrimaryWindow, Window};
use taffy::style::AvailableSpace;

fn scale_value(value: f32, factor: f64) -> f32 {
    (value as f64 * factor) as f32
}

#[derive(Clone)]
pub struct AutoTextMeasure {
    pub auto_text_info: TextShaper,
}

impl Measure for AutoTextMeasure {
    fn measure(
        &self,
        max_width: Option<f32>,
        max_height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func *");
        println!("max_width: {max_width:?}");
        println!("max_height: {max_height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let bounds = Vec2::new(
            max_width.unwrap_or_else(|| match available_width {
                AvailableSpace::Definite(x) => x,
                AvailableSpace::MaxContent => f32::INFINITY,
                AvailableSpace::MinContent => 0.,
            }),
            max_height.unwrap_or_else(|| match available_height {
                AvailableSpace::Definite(y) => y,
                AvailableSpace::MaxContent => f32::INFINITY,
                AvailableSpace::MinContent => 0.,
            }),
        );
        println!("bounds: {bounds:?}");
        let size = self.auto_text_info.compute_size(bounds);
        println!("size out: {size:?}");
        size
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


#[derive(Clone)]
pub struct AutoTextMeasure2 {
    pub auto_text_info: TextShaper,
}


#[derive(Clone)]
pub struct AutoTextMeasure3 {
    pub auto_text_info: TextShaper,
}

#[derive(Clone)]
pub struct AutoTextMeasure4 {
    pub auto_text_info: TextShaper,
}

impl Measure for AutoTextMeasure2 {
    fn measure(
        &self,
        max_width: Option<f32>,
        max_height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {max_width:?}");
        println!("max_height: {max_height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        
        use AvailableSpace::*;
        let bounds = match (max_width, max_height, available_width, available_height) {
            (_, _, MaxContent, _) | (_, _, _, MaxContent)  => Vec2::new(f32::INFINITY, f32::INFINITY),
            (_, _, MinContent, _) | (_, _, _, MinContent) => Vec2::new(0., f32::INFINITY),
            (None, Some(h), Definite(dw), Definite(_dh)) => Vec2::new(dw, h),
            (Some(w), None, Definite(_dw), Definite(dh)) => Vec2::new(w, dh),
            (None, None, Definite(dw), Definite(dh)) => Vec2::new(dw, dh),
            (Some(w), Some(h), Definite(_), Definite(_)) => Vec2::new(w, h), 
        };
        println!("bounds: {bounds:?}");
        let size = self.auto_text_info.compute_size(bounds);
        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


impl Measure for AutoTextMeasure3 {
    fn measure(
        &self,
        max_width: Option<f32>,
        max_height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {max_width:?}");
        println!("max_height: {max_height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");

        use AvailableSpace::*;
        let (mut width, mut height) = match (available_width, available_height) {
            (Definite(dw), Definite(dh)) => (dw, dh),
            (MaxContent, Definite(dh)) => (f32::INFINITY, dh),
            (MinContent, Definite(dh)) => (0., dh),
            (Definite(dw), MaxContent) => (dw, f32::INFINITY),
            (Definite(dw), MinContent) => (dw, 0.),
            (MaxContent, MaxContent) => (f32::INFINITY, f32::INFINITY),
            (MinContent, MinContent) => (0., 0.),
            (MaxContent, MinContent) => (f32::INFINITY, 0.),
            (MinContent, MaxContent) => (0., f32::INFINITY),
        };
    
        if let Some(max_w) = max_width {
            width = width.min(max_w);
        }
    
        if let Some(max_h) = max_height {
            height = height.min(max_h);
        }
    
        let bounds = Vec2::new(width, height);
        
        println!("bounds: {bounds:?}");
        let size = self.auto_text_info.compute_size(bounds);
        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


impl Measure for AutoTextMeasure4 {
    fn measure(
        &self,
        max_width: Option<f32>,
        max_height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {max_width:?}");
        println!("max_height: {max_height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");

        use AvailableSpace::*;
        let w = max_width.unwrap_or(f32::INFINITY);
        let h = max_height.unwrap_or(f32::INFINITY);
    
        let (mut width, mut height) = match (available_width, available_height) {
            (Definite(dw), Definite(dh)) => (dw, dh.min(h)),
            (MaxContent, Definite(dh)) => (f32::INFINITY, dh.min(h)),
            (MinContent, Definite(dh)) => (0., dh.min(h)),
            (Definite(dw), MaxContent) => (f32::INFINITY, f32::INFINITY),
            (Definite(dw), MinContent) => (0., f32::INFINITY),
            (MaxContent, MaxContent) => (f32::INFINITY, f32::INFINITY),
            (MinContent, MinContent) => (0., f32::INFINITY),
            (MaxContent, MinContent) => (f32::INFINITY, f32::INFINITY),
            (MinContent, MaxContent) => (f32::INFINITY, f32::INFINITY),
        };
        println!("width: {width:?}, height: {height:?}");
    
        let bounds = Vec2::new(width, height);
        
        println!("bounds: {bounds:?}");
        let size = self.auto_text_info.compute_size(bounds);
        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


#[derive(Clone)]
pub struct AutoTextMeasure5 {
    pub auto_text_info: TextShaper,
}


impl Measure for AutoTextMeasure5 {
    fn measure(
        &self,
        max_width: Option<f32>,
        max_height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {max_width:?}");
        println!("max_height: {max_height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");

        use AvailableSpace::*;
        let w = max_width.unwrap_or(f32::INFINITY);
        let h = max_height.unwrap_or(f32::INFINITY);

        let (mut width, mut height) = match (available_width, available_height) {
            (Definite(dw), Definite(dh)) => (dw, dh),
            (MaxContent, Definite(dh)) => (f32::INFINITY, dh),
            (MinContent, Definite(dh)) => (0., dh),
            (Definite(dw), MaxContent) => (dw, f32::INFINITY),
            (Definite(dw), MinContent) => (dw, 0.),
            (MaxContent, MaxContent) => (f32::INFINITY, f32::INFINITY),
            (MinContent, MinContent) => (0., 0.),
            (MaxContent, MinContent) => (f32::INFINITY, 0.),
            (MinContent, MaxContent) => (0., f32::INFINITY),
        };
    
        
        width = width.min(w);
        
    
        
        height = height.min(h);
        
    
        let bounds = Vec2::new(width, height);
        
        println!("bounds: {bounds:?}");
        let size = self.auto_text_info.compute_size(bounds);
        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


#[derive(Clone)]
pub struct AutoTextMeasure6 {
    pub auto_text_info: TextShaper,
}


impl Measure for AutoTextMeasure6 {
    fn measure(
        &self,
        max_width: Option<f32>,
        max_height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {max_width:?}");
        println!("max_height: {max_height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");

        use AvailableSpace::*;
        let w = max_width.unwrap_or(f32::INFINITY);
        let h = max_height.unwrap_or(f32::INFINITY);

        let (mut width, mut height) = match (available_width, available_height) {
            (Definite(dw), Definite(dh)) => (dw, dh),
            (MaxContent, Definite(dh)) => (f32::INFINITY, dh),
            (MinContent, Definite(dh)) => (0., dh),
            (Definite(dw), MaxContent) => (dw, f32::INFINITY),
            (Definite(dw), MinContent) => (dw, f32::INFINITY),
            (MaxContent, MaxContent) => (f32::INFINITY, f32::INFINITY),
            (MinContent, MinContent) => (0., f32::INFINITY),
            (MaxContent, MinContent) => (f32::INFINITY, 0.),
            (MinContent, MaxContent) => (0., f32::INFINITY),
        };
    
        
        width = width.min(w);
        
    
        
        height = height.min(h);
        
    
        let bounds = Vec2::new(width, height);
        
        println!("bounds: {bounds:?}");
        let size = self.auto_text_info.compute_size(bounds);
        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}



#[derive(Clone)]
pub struct AutoTextMeasure7 {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasure7 {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let mut bounds = Vec2::ZERO;

        let available: Vec2 =  match (available_width, available_height) {
            (AvailableSpace::Definite(w), AvailableSpace::Definite(h)) => (w, h).into(),
            (AvailableSpace::Definite(w), AvailableSpace::MinContent) => (w, self.info.min.y).into(),
            (AvailableSpace::Definite(w), AvailableSpace::MaxContent) => (w, self.info.max.y).into(),
            (AvailableSpace::MinContent, AvailableSpace::Definite(h)) => (self.info.min.x, h).into(),
            (AvailableSpace::MinContent, AvailableSpace::MinContent) => self.info.min,
            (AvailableSpace::MinContent, AvailableSpace::MaxContent) => (self.info.min.x, self.info.max.y).into(),
            (AvailableSpace::MaxContent, AvailableSpace::Definite(h)) => (self.info.max.x, h).into(),
            (AvailableSpace::MaxContent, AvailableSpace::MinContent) => (self.info.max.x, self.info.max.y).into(),
            (AvailableSpace::MaxContent, AvailableSpace::MaxContent) => (self.info.max.x, self.info.max.y).into(),
        };
        println!("available: {available:?}");
        bounds.x = width.map(|w| w.min(available.x)).unwrap_or(available.x) ;
        bounds.y = height.map(|h| h.min(available.y)).unwrap_or(available.y);
        let size = self.info.shaper.compute_size(available);
        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}



#[derive(Clone)]
pub struct AutoTextMeasure8 {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasure8 {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let mut bounds = Vec2::ZERO;

        use AvailableSpace::*;
       
        let size: Vec2 = match (available_width, available_height) {
            (Definite(w), MinContent) => (self.info.shaper.compute_size((w, f32::INFINITY).into()).x, self.info.min.y).into(),
            (Definite(w), MaxContent) => (self.info.shaper.compute_size((w, f32::INFINITY).into()).x, self.info.max.y).into(),
            (Definite(w), Definite(h)) => self.info.shaper.compute_size((w, h).into()),
            (MinContent, Definite(h)) => (self.info.min.x, self.info.shaper.compute_size((f32::INFINITY, h).into()).y).into(),
            (MaxContent, Definite(h)) => (self.info.max.x, self.info.shaper.compute_size((f32::INFINITY, h).into()).y).into(),
            (MinContent, MinContent) => self.info.min,
            (MinContent, MaxContent) => (self.info.min.x, self.info.max.y).into(),
            (MaxContent, MinContent) => (self.info.max.x, self.info.min.y).into(),
            (MaxContent, MaxContent) => self.info.max,
        };

        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


#[derive(Clone)]
pub struct AutoTextMeasure9 {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasure9 {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let mut bounds = Vec2::ZERO;

        use AvailableSpace::*;
       
        let size: Vec2 = match (width, height) {
            (None, None) => self.info.max,
            (None, Some(h)) => self.info.max,
            (Some(w), None) => self.info.shaper.compute_size((w, f32::INFINITY).into()),
            (Some(w), Some(h)) =>  self.info.shaper.compute_size((w, h).into()),
        };

        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}




#[derive(Clone)]
pub struct AutoTextMeasureX {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasureX {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func2 *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let mut bounds = Vec2::ZERO;

        use AvailableSpace::*;

        let min = self.info.min;
        let max = self.info.max;
        
        let size: Vec2 = match (width, height, available_width, available_height) {
            // (None, None, Definite(_), Definite(_)) => todo!(),
            // (None, None, Definite(_), MinContent) => todo!(),
            // (None, None, Definite(_), MaxContent) => todo!(),
            // (None, None, MinContent, Definite(_)) => todo!(),
            // (None, None, MinContent, MinContent) => todo!(),
            // (None, None, MinContent, MaxContent) => todo!(),
            // (None, None, MaxContent, Definite(_)) => todo!(),
            // (None, None, MaxContent, MinContent) => todo!(),
            // (None, None, MaxContent, MaxContent) => todo!(),
            // (None, Some(_), Definite(_), Definite(_)) => todo!(),
            // (None, Some(_), Definite(_), MinContent) => todo!(),
            // (None, Some(_), Definite(_), MaxContent) => todo!(),
            // (None, Some(_), MinContent, Definite(_)) => todo!(),
            // (None, Some(_), MinContent, MinContent) => todo!(),
            // (None, Some(_), MinContent, MaxContent) => todo!(),
            // (None, Some(_), MaxContent, Definite(_)) => todo!(),
            // (None, Some(_), MaxContent, MinContent) => todo!(),
            // (None, Some(_), MaxContent, MaxContent) => todo!(),
            // (Some(_), None, Definite(_), Definite(_)) => todo!(),
            // (Some(_), None, Definite(_), MinContent) => todo!(),
            // (Some(_), None, Definite(_), MaxContent) => todo!(),
            // (Some(_), None, MinContent, Definite(_)) => todo!(),
            // (Some(_), None, MinContent, MinContent) => todo!(),
            // (Some(_), None, MinContent, MaxContent) => todo!(),
            // (Some(_), None, MaxContent, Definite(_)) => todo!(),
            // (Some(_), None, MaxContent, MinContent) => todo!(),
            // (Some(_), None, MaxContent, MaxContent) => todo!(),
            // (Some(_), Some(_), Definite(_), Definite(_)) => todo!(),
            // (Some(_), Some(_), Definite(_), MinContent) => todo!(),
            // (Some(_), Some(_), Definite(_), MaxContent) => todo!(),
            // (Some(_), Some(_), MinContent, Definite(_)) => todo!(),
            // (Some(_), Some(_), MinContent, MinContent) => todo!(),
            // (Some(_), Some(_), MinContent, MaxContent) => todo!(),
            // (Some(_), Some(_), MaxContent, Definite(_)) => todo!(),
            // (Some(_), Some(_), MaxContent, MinContent) => todo!(),
            // (Some(_), Some(_), MaxContent, MaxContent) => todo!(),
            //(Some(w), _, _, _) => self.info.text_shaper.compute_size((w, f32::INFINITY).into()),
            //(None, None, MinContent, Definite(ah)) => self.info.text_shaper.compute_size((f32::INFINITY, ah).into()),
            //(Some(w), None, Definite(aw), MaxContent) => (self.info.text_shaper.compute_size((w, max.y).into()).x, max.y).into(),
            //(None, None, Definite(aw), MinContent) => (self.info.text_shaper.compute_size((aw, min.y).into()).x, min.y).into(),
            //(None, None, MinContent, MaxContent) => (min.x, max.y).into(),
            _ => max,
        };
        
        println!("size out: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}




#[derive(Clone)]
pub struct AutoTextMeasureOmega {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasureOmega {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func omega *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let min = self.info.min;
        let max = self.info.max;
        println!("min: {min:?}");
        println!("max: {max:?}");

        use AvailableSpace::*;

        
        
        let x =
            if let Some(width) = width {
                self.info.shaper.compute_size((width, f32::INFINITY).into()).x
            } else {
                match available_width {
                    Definite(w) => self.info.shaper.compute_size((w, f32::INFINITY).into()).x,
                    MinContent => min.x,
                    MaxContent => max.x,
                }
            };
        let y =
            if let Some(height) = height {
                min.y.min(height)
            } else {
                match available_height {
                    Definite(h) => min.y.min(h),
                    MinContent => min.y,
                    MaxContent => max.y,
                }
            };
        let size = Vec2::new(x, y);
        println!("size out: {size:?}");
        
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}



#[derive(Clone)]
pub struct AutoTextMeasureAlpha {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasureAlpha {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        mut available_width: AvailableSpace,
        mut available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func alpha *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let min = self.info.min;
        let max = self.info.max;
        println!("min: {min:?}");
        println!("max: {max:?}");

        use AvailableSpace::*;

        if let Some(width) = width {
            available_width = Definite(width);
        }

        if let Some(height) = height {
            available_height = Definite(height);
        }

        let size = 
            match (available_width, available_height) {
                (Definite(w), Definite(h)) => self.info.shaper.compute_size((w, h).into()),
                (Definite(w), MinContent) => self.info.shaper.compute_size((w, f32::INFINITY).into()).into(),
                (Definite(w), MaxContent) => self.info.shaper.compute_size((w, min.y).into()).into(),
                (MinContent, Definite(h)) => self.info.shaper.compute_size((min.x, h).into()).into(),
                (MinContent, MinContent) => min,
                (MinContent, MaxContent) => self.info.shaper.compute_size((min.x, f32::INFINITY).into()).into(),
                (MaxContent, Definite(h)) => self.info.shaper.compute_size((max.x, h).into()).into(),
                (MaxContent, MinContent) => max,
                (MaxContent, MaxContent) => max,
            };
        println!("size: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


#[derive(Clone)]
pub struct AutoTextMeasureBeta {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasureBeta {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        mut available_width: AvailableSpace,
        mut available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func beta *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let min = self.info.min;
        let max = self.info.max;
        println!("min: {min:?}");
        println!("max: {max:?}");

        use AvailableSpace::*;

        if let Some(width) = width {
            available_width = Definite(width);
        }

        if let Some(height) = height {
            available_height = Definite(height);
        }

        let axis = |available_space: AvailableSpace, min: f32, max: f32| {
            match available_space {
                Definite(w) => w,
                MinContent => min,
                MaxContent => max,
            }
        };

        let bounds = Vec2::new(
            axis(available_width, min.x, max.x),
            axis(available_height, max.y, min.y),
        );

        println!("bounds: {bounds:?}");
        let size = self.info.shaper.compute_size(bounds);
     
        println!("size: {size:?}");
        size.ceil()
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


#[derive(Clone)]
pub struct AutoTextMeasurePlus {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasurePlus {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func plus *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let min = self.info.min;
        let max = self.info.max;
        println!("min: {min:?}");
        println!("max: {max:?}");

        
        use AvailableSpace::*;
        let size =
            if let (Some(width), Some(height)) = (width, height) {
                Vec2::new(width, height)
            } else {
                let x =  width.unwrap_or_else(|| match available_width {
                    Definite(w) => w.min(max.x).max(min.x),
                    MinContent => max.x,
                    MaxContent => max.x,
                });
                
                let y = height.unwrap_or_else(|| 
                    self.info.shaper.compute_size((x, f32::INFINITY).into()).y
                );
                Vec2::new(x, y)
            }.ceil();
        println!("out: {size:?}");
        size
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}


#[derive(Clone)]
pub struct AutoTextMeasureR {
    pub info: AutoTextInfo,
}

impl Measure for AutoTextMeasureR {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func R *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let min = self.info.min;
        let max = self.info.max;
        println!("min: {min:?}");
        println!("max: {max:?}");

        
        use AvailableSpace::*;
        let size =
            match (width, height) {
                (Some(width), Some(height)) => {
                    Vec2::new(width, height)
                },
                (Some(width), None) => {
                    let y = match available_height {
                        Definite(h) => h.min(max.y).max(min.y),
                        MinContent => min.y,
                        MaxContent => max.y,
                    };                    
                    self.info.shaper.compute_size((width, y).into())
                },
                (None, Some(height)) => {
                    let _x = match available_width {
                        Definite(w) => w.min(max.x).max(min.x),
                        MinContent => min.x,
                        MaxContent => max.x,
                    };
                    self.info.shaper.compute_size((max.x, height).into())                  
                },
                _ => {
                    let x = match available_width {
                        Definite(w) => w.min(max.x).max(min.x),
                        MinContent => min.x,
                        MaxContent => max.x,
                    };

                    let y = match available_height {
                        Definite(h) => h.min(max.y).max(min.y),
                        MinContent => min.y,
                        MaxContent => max.y,
                    };
                    
                    self.info.shaper.compute_size((x, y).into())
                    //Vec2::new(x, y)
                }
            }.ceil();
        println!("out: {size:?}");
        size
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}

#[derive(Clone)]
pub struct AutoTextMeasureQ {
    pub info: AutoTextInfo,
}


impl Measure for AutoTextMeasureQ {
    fn measure(
        &self,
        width: Option<f32>,
        height: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!("\n* measure func Q *");
        println!("max_width: {width:?}");
        println!("max_height: {height:?}");
        println!("available_width: {available_width:?}");
        println!("available_height: {available_height:?}");
        let min = self.info.min;
        let max = self.info.max;
        println!("min: {min:?}");
        println!("max: {max:?}");

        
        use AvailableSpace::*;
        let size =
            match (width, height) {
                (Some(width), Some(height)) => {
                    Vec2::new(width, height)
                },
                (Some(width), None) => {
                    // let y = match available_height {
                    //     Definite(h) => h.min(max.y).max(min.y),
                    //     MinContent => min.y,
                    //     MaxContent => max.y,
                    // };                    
                    // self.info.shaper.compute_size((width, y).into())
                    (max.x, min.y).into()
                },
                (None, Some(height)) => {
                    (min.x, max.y).into()                 
                },
                _ => {
                    let x = match available_width {
                        Definite(w) => w.min(max.x).max(min.x),
                        MinContent => min.x,
                        MaxContent => max.x,
                    };

                    let y = match available_height {
                        Definite(h) => h.min(max.y).max(min.y),
                        MinContent => min.y,
                        MaxContent => max.y,
                    };
                    
                    self.info.shaper.compute_size((x, y).into())
                    //Vec2::new(x, y)
                }
            }.ceil();
        println!("out: {size:?}");
        size
    }

    fn dyn_clone(&self) -> Box<dyn Measure> {
        Box::new(self.clone())
    }
}

/// Creates a `Measure` for text nodes that allows the UI to determine the appropriate amount of space
/// to provide for the text given the fonts, the text itself and the constraints of the layout.
pub fn measure_text_system(
    mut queued_text: Local<Vec<Entity>>,
    mut last_scale_factor: Local<f64>,
    fonts: Res<Assets<Font>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    ui_scale: Res<UiScale>,
    mut text_pipeline: ResMut<TextPipeline>,
    mut text_queries: ParamSet<(
        Query<Entity, Changed<Text>>,
        Query<Entity, (With<Text>, With<Node>)>,
        Query<(&Text, &mut CalculatedSize)>,
    )>,
) {
    let window_scale_factor = windows
        .get_single()
        .map(|window| window.resolution.scale_factor())
        .unwrap_or(1.);

    let scale_factor = ui_scale.scale * window_scale_factor;

    #[allow(clippy::float_cmp)]
    if *last_scale_factor == scale_factor {
        // Adds all entities where the text or the style has changed to the local queue
        for entity in text_queries.p0().iter() {
            if !queued_text.contains(&entity) {
                queued_text.push(entity);
            }
        }
    } else {
        // If the scale factor has changed, queue all text
        for entity in text_queries.p1().iter() {
            queued_text.push(entity);
        }
        *last_scale_factor = scale_factor;
    }

    if queued_text.is_empty() {
        return;
    }

    let mut new_queue = Vec::new();
    let mut query = text_queries.p2();
    for entity in queued_text.drain(..) {
        if let Ok((text, mut calculated_size)) = query.get_mut(entity) {
            println!("\n* creating text measure *");
            match text_pipeline.compute_auto_text_measure(
                &fonts,
                &text.sections,
                scale_factor,
                text.alignment,
                text.linebreak_behaviour,
            ) {
                Ok(measure) => {
                    calculated_size.measure = Box::new(AutoTextMeasureQ {
                        info: measure,
                    });
                }
                Err(TextError::NoSuchFont) => {
                    new_queue.push(entity);
                }
                Err(e @ TextError::FailedToAddGlyph(_)) => {
                    panic!("Fatal error when processing text: {e}.");
                }
            };
        }
    }
    *queued_text = new_queue;
}


/// Updates the layout and size information whenever the text or style is changed.
/// This information is computed by the `TextPipeline` on insertion, then stored.
///
/// ## World Resources
///
/// [`ResMut<Assets<Image>>`](Assets<Image>) -- This system only adds new [`Image`] assets.
/// It does not modify or observe existing ones.
#[allow(clippy::too_many_arguments)]
pub fn text_system(
    mut queued_text: Local<Vec<Entity>>,
    mut textures: ResMut<Assets<Image>>,
    mut last_scale_factor: Local<f64>,
    fonts: Res<Assets<Font>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    text_settings: Res<TextSettings>,
    mut font_atlas_warning: ResMut<FontAtlasWarning>,
    ui_scale: Res<UiScale>,
    mut texture_atlases: ResMut<Assets<TextureAtlas>>,
    mut font_atlas_set_storage: ResMut<Assets<FontAtlasSet>>,
    mut text_pipeline: ResMut<TextPipeline>,
    mut text_queries: ParamSet<(
        Query<Entity, Or<(Changed<Text>, Changed<Node>)>>,
        Query<Entity, (With<Text>, With<Node>)>,
        Query<(&Node, &Text, &mut CalculatedSize, &mut TextLayoutInfo)>,
    )>,
) {
    // TODO: Support window-independent scaling: https://github.com/bevyengine/bevy/issues/5621
    let window_scale_factor = windows
        .get_single()
        .map(|window| window.resolution.scale_factor())
        .unwrap_or(1.);

    let scale_factor = ui_scale.scale * window_scale_factor;

    #[allow(clippy::float_cmp)]
    if *last_scale_factor == scale_factor {
        // Adds all entities where the text or the style has changed to the local queue
        for entity in text_queries.p0().iter() {
            if !queued_text.contains(&entity) {
                queued_text.push(entity);
            }
        }
    } else {
        // If the scale factor has changed, queue all text
        for entity in text_queries.p1().iter() {
            queued_text.push(entity);
        }
        *last_scale_factor = scale_factor;
    }

    let mut new_queue = Vec::new();
    let mut text_query = text_queries.p2();
    for entity in queued_text.drain(..) {
        if let Ok((node, text, mut calculated_size, mut text_layout_info)) =
            text_query.get_mut(entity)
        {
            println!("\n* processing text *");
            let node_size = Vec2::new(
                scale_value(node.size().x, scale_factor),
                scale_value(node.size().y, scale_factor),
            );
            println!("bounds: {:?}", node_size);
            match text_pipeline.queue_text(
                &fonts,
                &text.sections,
                scale_factor,
                text.alignment,
                text.linebreak_behaviour,
                node_size,
                &mut font_atlas_set_storage,
                &mut texture_atlases,
                &mut textures,
                text_settings.as_ref(),
                &mut font_atlas_warning,
                YAxisOrientation::TopToBottom,
            ) {
                Err(TextError::NoSuchFont) => {
                    // There was an error processing the text layout, let's add this entity to the
                    // queue for further processing
                    new_queue.push(entity);
                }
                Err(e @ TextError::FailedToAddGlyph(_)) => {
                    panic!("Fatal error when processing text: {e}.");
                }
                Ok(info) => {
                    println!("text layout size: {:?}", info.size);
                    *text_layout_info = info;
                }
            }
        }
    }
    *queued_text = new_queue;
}
