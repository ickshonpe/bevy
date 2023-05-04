use crate::{measurement::AvailableSpace, ContentSize, Measure, Node, UiImage};
use bevy_asset::Assets;
#[cfg(feature = "bevy_text")]
use bevy_ecs::query::Without;
use bevy_ecs::{
    prelude::Component,
    query::With,
    reflect::ReflectComponent,
    system::{Query, Res},
};
use bevy_math::Vec2;
use bevy_reflect::{std_traits::ReflectDefault, FromReflect, Reflect, ReflectFromReflect};
use bevy_render::texture::Image;
#[cfg(feature = "bevy_text")]
use bevy_text::Text;
use taffy::style_helpers::TaffyMinContent;

/// The size of the image in physical pixels
///
/// This field is set automatically by `update_image_calculated_size_system`
#[derive(Component, Debug, Copy, Clone, Default, Reflect, FromReflect)]
#[reflect(Component, Default, FromReflect)]
pub struct UiImageSize {
    size: Vec2,
}

impl UiImageSize {
    pub fn size(&self) -> Vec2 {
        self.size
    }
}

#[derive(Clone)]
pub struct ImageMeasure {
    // target size of the image
    size: Vec2,
}

fn resolve_constraints(constraint: Option<f32>, space: AvailableSpace) -> Option<f32> {
    constraint.or_else(|| match space {
        AvailableSpace::Definite(available_length) => Some(available_length),
        AvailableSpace::MinContent | AvailableSpace::MaxContent => None,
    })
}

impl Measure for ImageMeasure {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        println!();
        println!("size: {}", self.size);
        println!("w: {width_constraint:?}");
        println!("h: {height_constraint:?}");
        println!("sw: {available_width:?}");
        println!("sh: {available_height:?}");
        let w = resolve_constraints(width_constraint, available_width);
        let h = resolve_constraints(height_constraint, available_height);
        let out = match (w, h) {
            (Some(w), Some(h)) => Vec2::new(
                self.size.x.min(w), 
                self.size.y.min(h),
            ),
            (None, None) => Vec2::new(self.size.x, self.size.y),
            (Some(w), None) => Vec2::new(w, w * self.size.y / self.size.x),
            (None, Some(h)) => Vec2::new(h * self.size.x / self.size.y, h),
            
        };
        println!("out: {out}");
        out
    }
}


#[derive(Clone)]
pub struct ImageMeasure2 {
    // target size of the image
    size: Vec2,
}

#[derive(Debug)]
enum Sizing {
    MinContent,
    MaxContent,
}

impl From<AvailableSpace> for Sizing {
    fn from(value: AvailableSpace) -> Self {
        match value {
            AvailableSpace::Definite(_) => Sizing::MaxContent,
            AvailableSpace::MinContent => Sizing::MinContent,
            AvailableSpace::MaxContent => Sizing::MaxContent,
        }
    }
}

impl Measure for ImageMeasure2 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let sw = |w| Vec2::new(w, w * self.size.y / self.size.x);
        let sh = |h| Vec2::new(h * self.size.x / self.size.y, h);
        println!();
        println!("size: {}", self.size);
        println!("w: {width_constraint:?}");
        println!("h: {height_constraint:?}");
        println!("sw: {available_width:?}");
        println!("sh: {available_height:?}");
        let fit = |w, h, sizing: Sizing| -> Vec2 {
            let size_w = sw(w);
            let size_h = sh(h);
            if h < size_w.y {
                size_h
            } else if w < size_h.x {
                size_w
            } else {
                let area_w = size_w.x * size_w.y;
                let area_h = size_h.x * size_h.y;
                match sizing {
                    Sizing::MinContent => {
                        if area_w < area_h {
                            size_w
                        } else {
                            size_h
                        }
                    },
                    Sizing::MaxContent => {
                        if area_w < area_h {
                            size_h
                        } else {
                            size_w
                        }
                    },
                }

            }
        };
        let out = match (width_constraint, height_constraint, available_width, available_height) {
            (None, None, AvailableSpace::MinContent | AvailableSpace::MaxContent, AvailableSpace::MinContent | AvailableSpace::MaxContent) => self.size,
            (Some(w), Some(h), _, _) => Vec2::new(w, h),
            (None, None, AvailableSpace::Definite(w), AvailableSpace::Definite(h)) => {
                fit(w, h, Sizing::MaxContent)
            },
            (None, None, AvailableSpace::Definite(w), _) => sw(w),
            (None, None, _, AvailableSpace::Definite(h)) => sh(h),

            (None, Some(h), AvailableSpace::Definite(w), AvailableSpace::Definite(_)) => {
                fit(w, h, Sizing::MaxContent)
            },
            (Some(w), None, AvailableSpace::Definite(_), AvailableSpace::Definite(h)) => {
                fit(w, h, Sizing::MaxContent)
            },
            (None, Some(h), AvailableSpace::Definite(w), AvailableSpace::MinContent) => {
                fit(w, h, Sizing::MinContent)
            },
            (None, Some(h), AvailableSpace::Definite(w), AvailableSpace::MaxContent) => {
                fit(w, h, Sizing::MaxContent)
            },
            (Some(w), None, _, AvailableSpace::MinContent | AvailableSpace::MaxContent) => sw(w),
            (None, Some(h), AvailableSpace::MinContent | AvailableSpace::MaxContent, _) => sh(h),
            (Some(w), None, AvailableSpace::MinContent, AvailableSpace::Definite(h)) => 
                fit(w, h, Sizing::MinContent),
            (Some(w), None, AvailableSpace::MaxContent, AvailableSpace::Definite(h)) =>
                fit(w, h, Sizing::MaxContent),
        };
        println!("out: {out}");
        out
    }
}

#[derive(Clone)]
pub struct ImageMeasure3 {
    // target size of the image
    size: Vec2,
}


impl Measure for ImageMeasure3 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let sw = |w| Vec2::new(w, w * self.size.y / self.size.x);
        let sh = |h| Vec2::new(h * self.size.x / self.size.y, h);
        println!();
        println!("size: {}", self.size);
        println!("w: {width_constraint:?}");
        println!("h: {height_constraint:?}");
        println!("sw: {available_width:?}");
        println!("sh: {available_height:?}");
        let fit = |w, h, sizing: Sizing| -> Vec2 {
            println!("fit [{w}, {h}] with {sizing:?}");
            let size_w = sw(w);
            let size_h = sh(h);
            println!("size based on width: {size_w}");
            println!("size based on height: {size_h}");
            if h < size_w.y {
                println!("size based on width does not fit, choose height based");
                size_h
            } else if w < size_h.x {
                println!("size based on height does not fit, choose width based");
                size_w
            } else {
                println!("both fitting");
                let area_w = size_w.x * size_w.y;
                let area_h = size_h.x * size_h.y;
                match sizing {
                    Sizing::MinContent => {
                        if area_w < area_h {
                            size_w
                        } else {
                            size_h
                        }
                    },
                    Sizing::MaxContent => {
                        if area_w < area_h {
                            size_h
                        } else {
                            size_w
                        }
                    },
                }

            }
        };
        let out = match (width_constraint, height_constraint, available_width, available_height) {
            (None, None, AvailableSpace::MinContent | AvailableSpace::MaxContent, AvailableSpace::MinContent | AvailableSpace::MaxContent) => self.size,
            (Some(w), Some(h), _, _) => fit(w, h, Sizing::MaxContent),
            (None, None, AvailableSpace::Definite(w), AvailableSpace::Definite(h)) => {
                fit(w, h, Sizing::MaxContent)
            },
            (None, None, AvailableSpace::Definite(w), AvailableSpace::MinContent) => 
                fit(w, self.size.y, Sizing::MinContent),
            (None, None, AvailableSpace::Definite(w), AvailableSpace::MaxContent) =>
                fit(w, self.size.y, Sizing::MaxContent),
            (None, None, AvailableSpace::MinContent, AvailableSpace::Definite(h)) =>
                fit(self.size.x, h, Sizing::MinContent),
            (None, None, AvailableSpace::MaxContent, AvailableSpace::Definite(h)) => 
                fit(self.size.x, h, Sizing::MaxContent),

            (None, Some(h), AvailableSpace::Definite(w), AvailableSpace::Definite(_)) => {
                fit(w, h, Sizing::MaxContent)
            },
            (Some(w), None, AvailableSpace::Definite(_), AvailableSpace::Definite(h)) => {
                fit(w, h, Sizing::MaxContent)
            },
            (None, Some(h), AvailableSpace::Definite(w), AvailableSpace::MinContent) => {
                fit(w, h, Sizing::MinContent)
            },
            (None, Some(h), AvailableSpace::Definite(w), AvailableSpace::MaxContent) => {
                fit(w, h, Sizing::MaxContent)
            },
            (Some(w), None, _, AvailableSpace::MinContent | AvailableSpace::MaxContent) => sw(w),
            (None, Some(h), AvailableSpace::MinContent | AvailableSpace::MaxContent, _) => sh(h),
            (Some(w), None, AvailableSpace::MinContent, AvailableSpace::Definite(h)) => 
                fit(w, h, Sizing::MinContent),
            (Some(w), None, AvailableSpace::MaxContent, AvailableSpace::Definite(h)) =>
                fit(w, h, Sizing::MaxContent),
        };
        println!("out: {out}");
        out
    }
}

#[derive(Clone)]
pub struct ImageMeasure4 {
    // target size of the image
    size: Vec2,
}


impl Measure for ImageMeasure4 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let sw = |w| Vec2::new(w, w * self.size.y / self.size.x);
        let sh = |h| Vec2::new(h * self.size.x / self.size.y, h);
        println!();
        println!("size: {}", self.size);
        println!("w: {width_constraint:?}");
        println!("h: {height_constraint:?}");
        println!("sw: {available_width:?}");
        println!("sh: {available_height:?}");
        let fit = |w, h, w_sizing: Sizing, h_sizing: Sizing| -> Vec2 {
            println!("fit [{w}, {h}] with {w_sizing:?}, {h_sizing:?}");
            let size_w = sw(w);
            let size_h = sh(h);
            println!("size based on width: {size_w}");
            println!("size based on height: {size_h}");
            if h < size_w.y {
                println!("size based on width does not fit, choose height based");
                size_h
            } else if w < size_h.x {
                println!("size based on height does not fit, choose width based");
                size_w
            } else {
                println!("both fitting");
                // match sizing {
                //     Sizing::MinContent => {
                //         if area_w < area_h {
                //             size_w
                //         } else {
                //             size_h
                //         }
                //     },
                //     Sizing::MaxContent => {
                //         if area_w < area_h {
                //             size_h
                //         } else {
                //             size_w
                //         }
                //     },
                // }
                Vec2::new(
                    match w_sizing {
                        Sizing::MinContent => size_w.x.min(size_h.x),
                        Sizing::MaxContent => size_w.x.max(size_h.x),
                    },
                    match h_sizing {
                        Sizing::MinContent => size_w.y.min( size_h.y),
                        Sizing::MaxContent => size_w.y.max(size_h.y),
                    },

                )

            }
        };
        let out = match (width_constraint, height_constraint, available_width, available_height) {
            (None, None, AvailableSpace::MinContent | AvailableSpace::MaxContent, AvailableSpace::MinContent | AvailableSpace::MaxContent) => self.size,
            (Some(w), Some(h), aw, ah) => fit(w, h, aw.into(), ah.into()),
            (None, None, AvailableSpace::Definite(w), AvailableSpace::Definite(h)) => {
                fit(w, h, Sizing::MaxContent, Sizing::MaxContent)
            },
            (None, None, AvailableSpace::Definite(w), ah) => 
                fit(w, self.size.y, Sizing::MaxContent, ah.into()),
            
            (None, None, aw, AvailableSpace::Definite(h)) =>
                fit(self.size.x, h, aw.into(), Sizing::MaxContent),

            (None, Some(h), AvailableSpace::Definite(w), AvailableSpace::Definite(_)) => {
                fit(w, h, Sizing::MaxContent, Sizing::MaxContent)
            },
            (Some(w), None, AvailableSpace::Definite(_), AvailableSpace::Definite(h)) => {
                fit(w, h, Sizing::MaxContent, Sizing::MaxContent)
            },
            (None, Some(h), AvailableSpace::Definite(w), ah) => {
                fit(w, h, Sizing::MinContent, ah.into())
            },
            (Some(w), None, _, AvailableSpace::MinContent | AvailableSpace::MaxContent) => sw(w),
            (None, Some(h), AvailableSpace::MinContent | AvailableSpace::MaxContent, _) => sh(h),
            (Some(w), None, aw, AvailableSpace::Definite(h)) => 
                fit(w, h, aw.into(), Sizing::MinContent),
        };
        println!("out: {out}");
        out
    }
}


#[derive(Clone)]
pub struct ImageMeasure5 {
    // target size of the image
    size: Vec2,
}


impl Measure for ImageMeasure5 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let sw = |w| Vec2::new(w, w * self.size.y / self.size.x);
        let sh = |h| Vec2::new(h * self.size.x / self.size.y, h);
        println!();
        println!("size: {}", self.size);
        println!("w: {width_constraint:?}");
        println!("h: {height_constraint:?}");
        println!("sw: {available_width:?}");
        println!("sh: {available_height:?}");
        let fit = |w, h, w_sizing: AvailableSpace, h_sizing: AvailableSpace| -> Vec2 {
            println!("fit [{w}, {h}] with {w_sizing:?}, {h_sizing:?}");
            let size_w = sw(w);
            let size_h = sh(h);
            println!("size based on width: {size_w}");
            println!("size based on height: {size_h}");
            if h < size_w.y {
                println!("size based on width does not fit, choose height based");
                size_h
            } else if w < size_h.x {
                println!("size based on height does not fit, choose width based");
                size_w
            } else {
                println!("both fitting");
                // match sizing {
                //     Sizing::MinContent => {
                //         if area_w < area_h {
                //             size_w
                //         } else {
                //             size_h
                //         }
                //     },
                //     Sizing::MaxContent => {
                //         if area_w < area_h {
                //             size_h
                //         } else {
                //             size_w
                //         }
                //     },
                // }
                Vec2::new(
                    match w_sizing {
                        AvailableSpace::MinContent => size_w.x.min(size_h.x),
                        _ => size_w.x.max(size_h.x),
                    },
                    match h_sizing {
                        AvailableSpace::MinContent => size_w.y.min( size_h.y),
                        _ => size_w.y.max(size_h.y),
                    },

                )

            }
        };
        let out = match (width_constraint, height_constraint, available_width, available_height) {
            (None, None, AvailableSpace::MinContent | AvailableSpace::MaxContent, AvailableSpace::MinContent | AvailableSpace::MaxContent) => self.size,
            (Some(w), Some(h), aw, ah) => fit(w, h, aw.into(), ah.into()),
            (None, None, AvailableSpace::Definite(w), AvailableSpace::Definite(h)) => {
                fit(w, h, available_width, available_height)
            },
            (None, None, AvailableSpace::Definite(w), ah) => 
                fit(w, self.size.y, available_width, available_height),
            
            (None, None, aw, AvailableSpace::Definite(h)) =>
                fit(self.size.x, h, available_width, available_height),

            (None, Some(h), AvailableSpace::Definite(w), AvailableSpace::Definite(_)) => {
                fit(w, h, available_width, available_height)
            },
            (Some(w), None, AvailableSpace::Definite(_), AvailableSpace::Definite(h)) => {
                fit(w, h, available_width, available_height)
            },
            (None, Some(h), AvailableSpace::Definite(w), ah) => {
                fit(w, h, available_width, available_height)
            },
            (Some(w), None, _, AvailableSpace::MinContent | AvailableSpace::MaxContent) => sw(w),
            (None, Some(h), AvailableSpace::MinContent | AvailableSpace::MaxContent, _) => sh(h),
            (Some(w), None, aw, AvailableSpace::Definite(h)) => 
                fit(w, h, available_width, available_height),
        };
        println!("out: {out}");
        out
    }
}


#[derive(Clone)]
pub struct ImageMeasure6 {
    // target size of the image
    size: Vec2,
}


impl Measure for ImageMeasure6 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let sw = |w| Vec2::new(w, w * self.size.y / self.size.x);
        let sh = |h| Vec2::new(h * self.size.x / self.size.y, h);
        println!();
        println!("size: {}", self.size);
        println!("w: {width_constraint:?}");
        println!("h: {height_constraint:?}");
        println!("sw: {available_width:?}");
        println!("sh: {available_height:?}");
        let fit = |w, h, w_sizing: AvailableSpace, h_sizing: AvailableSpace| -> Vec2 {
            println!("fit [{w}, {h}] with {w_sizing:?}, {h_sizing:?}");
            let size_w = sw(w);
            let size_h = sh(h);
            println!("size based on width: {size_w}");
            println!("size based on height: {size_h}");
            if h < size_w.y {
                println!("size based on width does not fit, choose height based");
                size_h
            } else if w < size_h.x {
                println!("size based on height does not fit, choose width based");
                size_w
            } else {
                println!("both fitting");
                Vec2::new(
                    match w_sizing {
                        AvailableSpace::MinContent => size_w.x.min(size_h.x),
                        _ => size_w.x.max(size_h.x),
                    },
                    match h_sizing {
                        AvailableSpace::MinContent => size_w.y.min( size_h.y),
                        _ => size_w.y.max(size_h.y),
                    },

                )

            }
        };
        let w = width_constraint.unwrap_or(match available_width {
            AvailableSpace::Definite(w) => w,
            AvailableSpace::MinContent => self.size.x,
            AvailableSpace::MaxContent => self.size.x,
        });

        let h = height_constraint.unwrap_or(match available_height {
            AvailableSpace::Definite(h) => h,
            AvailableSpace::MinContent => self.size.y,
            AvailableSpace::MaxContent => self.size.y,
        });

        let out = fit(w, h, available_width, available_height);

        println!("out: {out}");
        out
    }
}

#[derive(Clone)]
pub struct ImageMeasure8 {
    // target size of the image
    size: Vec2,
}

impl Measure for ImageMeasure8 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let sw = |w| Vec2::new(w, w * self.size.y / self.size.x);
        let sh = |h| Vec2::new(h * self.size.x / self.size.y, h);
        let width = width_constraint.unwrap_or_else(|| match available_width {
            AvailableSpace::Definite(w) => w,
            _ => self.size.x,
        });

        let height = height_constraint.unwrap_or_else(|| match available_height {
            AvailableSpace::Definite(h) => h,
            _ => self.size.y,
        });
    
        let size_w = sw(width);
        let size_h = sh(height);

        let size = if height < size_w.y {
            size_h
        } else if width < size_h.x {
            size_w
        } else {
            Vec2::new(
                match available_width {
                    AvailableSpace::MinContent => size_w.x.min(size_h.x),
                    _ => size_w.x.max(size_h.x),
                },
                match available_height {
                    AvailableSpace::MinContent => size_w.y.min(size_h.y),
                    _ => size_w.y.max(size_h.y),
                },
            )
        };

        size
    }
}


#[derive(Clone)]
pub struct ImageMeasure7 {
    // target size of the image
    size: Vec2,
}


impl Measure for ImageMeasure7 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let sw = |w| Vec2::new(w, w * self.size.y / self.size.x);
        let sh = |h| Vec2::new(h * self.size.x / self.size.y, h);
        println!();
        println!("size: {}", self.size);
        println!("w: {width_constraint:?}");
        println!("h: {height_constraint:?}");
        println!("sw: {available_width:?}");
        println!("sh: {available_height:?}");
        let fit = |w, h, w_sizing: AvailableSpace, h_sizing: AvailableSpace| -> Vec2 {
            println!("fit [{w}, {h}] with {w_sizing:?}, {h_sizing:?}");
            let size_w = sw(w);
            let size_h = sh(h);
            println!("size based on width: {size_w}");
            println!("size based on height: {size_h}");
           
                Vec2::new(
                    match w_sizing {
                        AvailableSpace::MinContent => size_w.x.min(size_h.x),
                        _ => size_w.x.max(size_h.x),
                    },
                    match h_sizing {
                        AvailableSpace::MinContent => size_w.y.min( size_h.y),
                        _ => size_w.y.max(size_h.y),
                    },

                )

        };
        let w = width_constraint.unwrap_or(match available_width {
            AvailableSpace::Definite(w) => w,
            AvailableSpace::MinContent => self.size.x,
            AvailableSpace::MaxContent => self.size.x,
        });

        let h = height_constraint.unwrap_or(match available_height {
            AvailableSpace::Definite(h) => h,
            AvailableSpace::MinContent => self.size.y,
            AvailableSpace::MaxContent => self.size.y,
        });

        let out = fit(w, h, available_width, available_height);

        println!("out: {out}");
        out
    }
}

#[derive(Clone)]
pub struct ImageMeasure9 {
    // target size of the image
    size: Vec2,
}


impl Measure for ImageMeasure9 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let sw = |w| Vec2::new(w, w * self.size.y / self.size.x);
        let sh = |h| Vec2::new(h * self.size.x / self.size.y, h);

        let width = match width_constraint {
            Some(w) => w,
            None => match available_width {
                AvailableSpace::Definite(w) => w,
                _ => self.size.x,
            },
        };

        let height = match height_constraint {
            Some(h) => h,
            None => match available_height {
                AvailableSpace::Definite(h) => h,
                _ => self.size.y,
            },
        };

        let size_by_width = sw(width);
        let size_by_height = sh(height);

        let (new_width, new_height) = if size_by_width.y <= height && size_by_width.x <= width {
            (size_by_width.x, size_by_width.y)
        } else if size_by_height.x <= width && size_by_height.y <= height {
            (size_by_height.x, size_by_height.y)
        } else {
            // Neither dimensions fit within the constraints, we pick the largest dimension that is still within the constraint
            if size_by_width.y > height && size_by_height.x > width {
                // Both dimensions are larger than constraints, we choose the one with the smallest area that is outside the constraint
                if (size_by_width.y - height) * size_by_width.x < (size_by_height.x - width) * size_by_height.y {
                    (size_by_width.x, height)
                } else {
                    (width, size_by_height.y)
                }
            } else if size_by_width.y > height {
                // Width-based size exceeds height constraint
                (width, size_by_height.y)
            } else {
                // Height-based size exceeds width constraint
                (size_by_width.x, height)
            }
        };

        Vec2::new(new_width, new_height)
    }
}
#[derive(Clone)]
pub struct ImageMeasure10 {
    // target size of the image
    size: Vec2,
}
impl Measure for ImageMeasure10 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let aspect_ratio = self.size.x / self.size.y;

        let width = match width_constraint {
            Some(w) => w,
            None => match available_width {
                AvailableSpace::Definite(w) => w,
                _ => self.size.x,
            },
        };

        let height = match height_constraint {
            Some(h) => h,
            None => match available_height {
                AvailableSpace::Definite(h) => h,
                _ => self.size.y,
            },
        };

        let target_width = width.min(height * aspect_ratio);
        let target_height = height.min(target_width / aspect_ratio);

        Vec2::new(target_width, target_height)
    }
}

struct ImageMeasure11 {
    size: Vec2,
}

impl Measure for ImageMeasure11 {
    fn measure(
        &self,
        width_constraint: Option<f32>,
        height_constraint: Option<f32>,
        available_width: AvailableSpace,
        available_height: AvailableSpace,
    ) -> Vec2 {
        let aspect_ratio = self.size.x / self.size.y;

        let (mut target_width, mut target_height) = match (width_constraint, height_constraint) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, w / aspect_ratio),
            (None, Some(h)) => (h * aspect_ratio, h),
            (None, None) => match (available_width, available_height) {
                (AvailableSpace::Definite(w), AvailableSpace::Definite(h)) => (w, h),
                (AvailableSpace::Definite(w), _) => (w, w / aspect_ratio),
                (_, AvailableSpace::Definite(h)) => (h * aspect_ratio, h),
                _ => (self.size.x, self.size.y),
            },
        };

        match available_width {
            AvailableSpace::Definite(max_width) => {
                if target_width > max_width {
                    target_width = max_width;
                    target_height = target_width / aspect_ratio;
                }
            }
            _ => {}
        }

        match available_height {
            AvailableSpace::Definite(max_height) => {
                if target_height > max_height {
                    target_height = max_height;
                    target_width = target_height * aspect_ratio;
                }
            }
            _ => {}
        }

        Vec2::new(target_width, target_height)
    }
}

/// Updates content size of the node based on the image provided
pub fn update_image_content_size_system(
    textures: Res<Assets<Image>>,
    #[cfg(feature = "bevy_text")] mut query: Query<
        (&mut ContentSize, &UiImage, &mut UiImageSize),
        (With<Node>, Without<Text>),
    >,
    #[cfg(not(feature = "bevy_text"))] mut query: Query<
        (&mut ContentSize, &UiImage, &mut UiImageSize),
        With<Node>,
    >,
) {
    for (mut content_size, image, mut image_size) in &mut query {
        if let Some(texture) = textures.get(&image.texture) {
            let size = Vec2::new(
                texture.texture_descriptor.size.width as f32,
                texture.texture_descriptor.size.height as f32,
            );
            // Update only if size has changed to avoid needless layout calculations
            if size != image_size.size {
                image_size.size = size;
                content_size.set(ImageMeasure { size });
            }
        }
    }
}
