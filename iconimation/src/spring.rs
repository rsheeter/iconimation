//! Spring-based animation, ported from [Android's implmentation](https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/)

use std::str::FromStr;

use crate::error::SpringBuildError;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Spring {
    Overdamped {
        gamma_plus: f64,
        gamma_minus: f64,
    },
    CriticallyDamped {
        natural_freq: f64,
    },
    Underdamped {
        damping: f64,
        natural_freq: f64,
        damped_freq: f64,
    },
}

impl Spring {
    pub fn new(damping: f64, stiffness: f64) -> Result<Self, SpringBuildError> {
        if damping < 0.0 {
            return Err(SpringBuildError::InvalidDamping);
        }
        Ok(Self::new_internal(damping, stiffness))
    }

    /// Precompute values we need repeatedly
    ///
    /// <https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/SpringForce.java;l=247-256;drc=d43dfb63eaf6cf07414c0a6a556f4f5881fa9fad>
    fn new_internal(damping: f64, stiffness: f64) -> Self {
        let natural_freq = stiffness.sqrt();
        if damping > 1.0 {
            Self::Overdamped {
                gamma_plus: -damping * natural_freq
                    + natural_freq * (damping * damping - 1.0).sqrt(),
                gamma_minus: -damping * natural_freq
                    - natural_freq * (damping * damping - 1.0).sqrt(),
            }
        } else if damping < 1.0 {
            Self::Underdamped {
                damping,
                natural_freq,
                damped_freq: natural_freq * (1.0 - damping * damping).sqrt(),
            }
        } else {
            Self::CriticallyDamped { natural_freq }
        }
    }

    pub fn standard() -> Self {
        Self::new_internal(1.0, 380.0)
    }
    pub fn smooth_spatial() -> Self {
        Self::new_internal(1.0, 190.0)
    }
    pub fn smooth_non_spatial() -> Self {
        Self::new_internal(1.0, 380.0)
    }
    pub fn expressive_spatial() -> Self {
        Self::new_internal(0.8, 380.0)
    }
    pub fn expressive_non_spatial() -> Self {
        Self::new_internal(1.0, 380.0)
    }

    /// Compute for a new time, such as a new frame
    ///
    /// See:
    /// * [DynamicAnimation::doAnimationFrame](https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/DynamicAnimation.java;l=663-693;drc=b7d26a383dbb3c7fa3f276d8ad1afdac5bb5443f)
    /// * [SpringForce::updateValues](https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/SpringForce.java;l=261-307;drc=b7d26a383dbb3c7fa3f276d8ad1afdac5bb5443f)
    pub fn update(&self, time: f64, last: AnimatedValue) -> AnimatedValue {
        let delta_t = time - last.time;
        let last_displacement = last.value - last.final_value;

        let (value, velocity) = match self {
            Spring::Overdamped {
                gamma_plus,
                gamma_minus,
            } => {
                let coeff_a = last_displacement
                    - (gamma_minus * last_displacement - last.velocity)
                        / (gamma_minus - gamma_plus);
                let coeff_b =
                    (gamma_minus * last_displacement - last.velocity) / (gamma_minus - gamma_plus);
                let value = coeff_a * (gamma_minus * delta_t).exp()
                    + coeff_b * (gamma_plus * delta_t).exp();
                let velocity = coeff_a * gamma_minus * (gamma_minus * delta_t).exp()
                    + coeff_b * gamma_plus * (gamma_plus * delta_t).exp();
                (value, velocity)
            }
            Spring::CriticallyDamped { natural_freq } => {
                let coeff_a = last_displacement;
                let coeff_b = last.velocity + natural_freq * last_displacement;
                let value = (coeff_a + coeff_b * delta_t) * (-natural_freq * delta_t).exp();
                let velocity =
                    (coeff_a + coeff_b * delta_t) * (-natural_freq * delta_t).exp() * -natural_freq
                        + coeff_b * (-natural_freq * delta_t).exp();
                (value, velocity)
            }
            Spring::Underdamped {
                damping,
                natural_freq,
                damped_freq,
            } => {
                let cos_coeff = last_displacement;
                let sin_coeff = (1.0 / damped_freq)
                    * (damping * natural_freq * last_displacement + last.velocity);
                let value = (-damping * natural_freq * delta_t).exp()
                    * (cos_coeff * (damped_freq * delta_t).cos()
                        + sin_coeff * (damped_freq * delta_t.sin()));
                let velocity = value * -natural_freq * damping
                    + (-damping * natural_freq * delta_t).exp()
                        * (-damped_freq * cos_coeff * (damped_freq * delta_t).sin()
                            + damped_freq * sin_coeff * (damped_freq * delta_t).cos());
                (value, velocity)
            }
        };
        AnimatedValue {
            value: value + last.final_value,
            velocity,
            final_value: last.final_value,
            time,
            value_type: last.value_type,
        }
    }
}

impl FromStr for Spring {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "standard" => Ok(Spring::standard()),
            "smooth-spatial" => Ok(Spring::smooth_spatial()),
            "smooth-non-spatial" => Ok(Spring::smooth_non_spatial()),
            "expressive-spatial" => Ok(Spring::expressive_spatial()),
            "expressive-non-spatial" => Ok(Spring::expressive_non_spatial()),
            _ => Err(()),
        }
    }
}

/// The state of something being animated
///
/// <https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/DynamicAnimation.java;l=332-336;drc=d43dfb63eaf6cf07414c0a6a556f4f5881fa9fad>
/// <https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/SpringForce.java;l=261-307;drc=b7d26a383dbb3c7fa3f276d8ad1afdac5bb5443f>
#[derive(Debug, Copy, Clone)]
pub struct AnimatedValue {
    pub value: f64,
    pub value_type: AnimatedValueType,
    pub velocity: f64,
    pub final_value: f64,
    pub time: f64,
}

impl AnimatedValue {
    pub fn new(from: f64, to: f64, value_type: AnimatedValueType) -> Self {
        AnimatedValue {
            value: from,
            velocity: 0.0,
            final_value: to,
            time: 0.0,
            value_type,
        }
    }

    /// <https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/SpringForce.java;l=221-228;drc=b7d26a383dbb3c7fa3f276d8ad1afdac5bb5443f>
    pub fn is_at_equilibrium(&self) -> bool {
        let thresholds = self.value_type.thresholds();
        self.velocity.abs() < thresholds.velocity_threshold
            && (self.value - self.final_value).abs() < thresholds.value_threshold
    }
}

struct ValueThresholds {
    value_threshold: f64,
    velocity_threshold: f64,
}

/// We need to know the value type because it influences associated constants
#[derive(Debug, Copy, Clone)]
pub enum AnimatedValueType {
    Rotation,
    Scale,
    Position,
    Custom { value_threshold: f64 },
}

// TODO: type specific values
impl AnimatedValueType {
    /// This multiplier is used to calculate the velocity threshold given a certain value threshold.
    /// The idea is that if it takes >= 1 frame to move the value threshold amount, then the velocity
    /// is a reasonable threshold.
    ///
    /// <https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/SpringForce.java;l=76-79;drc=b7d26a383dbb3c7fa3f276d8ad1afdac5bb5443f>
    const VELOCITY_THRESHOLD_MULTIPLIER: f64 = 1000.0 / 16.0;

    fn thresholds(&self) -> ValueThresholds {
        // Values based on <https://cs.android.com/android/platform/superproject/main/+/main:frameworks/base/core/java/com/android/internal/dynamicanimation/animation/DynamicAnimation.java>
        let value_threshold = match self {
            AnimatedValueType::Position => 0.01, // Android uses MIN_VISIBLE_CHANGE_PIXELS = 1f; but we don't know our pixel size
            AnimatedValueType::Rotation => 0.1, // Android uses MIN_VISIBLE_CHANGE_ROTATION_DEGREES = 1f / 10f;
            AnimatedValueType::Scale => 1.0 / 500.0, // Android uses MIN_VISIBLE_CHANGE_SCALE = 1f / 500f;
            AnimatedValueType::Custom { value_threshold } => *value_threshold,
        } * 0.75; // Android multiplies by THRESHOLD_MULTIPLIER = 0.75f;
        let velocity_threshold = value_threshold * Self::VELOCITY_THRESHOLD_MULTIPLIER;
        ValueThresholds {
            value_threshold,
            velocity_threshold,
        }
    }
}

#[cfg(test)]
mod tests {
    use ordered_float::OrderedFloat;

    use crate::spring::AnimatedValueType;

    use super::AnimatedValue;
    use super::Spring;

    #[test]
    fn from_zero_to_100() {
        let spring = Spring::expressive_spatial();

        // 60fps, run until complete or 5s
        let mut animated_value = AnimatedValue::new(0.0, 100.0, AnimatedValueType::Scale);
        let mut frame_values = Vec::new();
        for frame in 0..300 {
            let time = frame as f64 / 60.0;
            animated_value = spring.update(time, animated_value);
            frame_values.push(animated_value);
            if animated_value.is_at_equilibrium() {
                break;
            }
        }

        assert!(
            frame_values.len() < 50,
            "Should finish within 50 frames\n{frame_values:#?}"
        );
        assert!(
            frame_values
                .iter()
                .map(|v| OrderedFloat(v.value))
                .max()
                .unwrap()
                .0
                > 100.0,
            "Should overshoot\n{frame_values:#?}"
        );
        assert!(
            frame_values.first().unwrap().value == 0.0,
            "Should start at the beginning\n{frame_values:#?}"
        );
        assert!(
            (frame_values.last().unwrap().value - 100.0).abs() < 0.001,
            "Should end very near the end\n{frame_values:#?}"
        );
    }
}
