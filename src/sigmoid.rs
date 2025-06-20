use std::env;

/// Sigmoid function configuration
#[derive(Debug, Clone, Copy)]
pub struct SigmoidFn {
    /// Sigmoid function midpoint (x0)
    pub midpoint: f32,
    /// Sigmoid function steepness (k)
    pub steepness: f32,
}

impl SigmoidFn {
    /// Evaluate the sigmoid function
    ///
    /// # Arguments
    ///
    /// * `x` - Input value
    ///
    /// # Returns
    ///
    /// Sigmoid function result in range [0, 1]
    pub fn evaluate(self, x: f32) -> f32 {
        let exp_arg = -self.steepness * (x - self.midpoint);
        1.0 / (1.0 + exp_arg.exp())
    }
}

impl Default for SigmoidFn {
    fn default() -> Self {
        Self {
            midpoint: 0.5,
            steepness: 8.0,
        }
    }
}

// ================================
// Environment variable helper for SigmoidFn configuration
// ================================

/// Create SigmoidFn from environment variables with fallback to defaults
pub(crate) fn get_sigmoid_config(
    env_prefix: &str,
    default_midpoint: f32,
    default_steepness: f32,
) -> SigmoidFn {
    let midpoint_env = format!("{env_prefix}_MIDPOINT");
    let steepness_env = format!("{env_prefix}_STEEPNESS");

    let midpoint = env::var(&midpoint_env)
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(default_midpoint);

    let steepness = env::var(&steepness_env)
        .ok()
        .and_then(|s| s.parse::<f32>().ok())
        .unwrap_or(default_steepness);

    SigmoidFn {
        midpoint,
        steepness,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sigmoid_basic() {
        let f = SigmoidFn {
            midpoint: 0.5,
            steepness: 10.0,
        };

        // Test basic functionality
        let result = f.evaluate(0.5);
        assert!((result - 0.5).abs() < 0.01); // Should be close to 0.5 at midpoint

        // Test range boundaries
        let low_result = f.evaluate(0.0);
        let high_result = f.evaluate(1.0);
        assert!(low_result < 0.5);
        assert!(high_result > 0.5);

        // Results should be in [0, 1] range
        assert!((0.0..=1.0).contains(&low_result));
        assert!((0.0..=1.0).contains(&high_result));
    }

    #[test]
    fn test_sigmoid_monotonic() {
        let f = SigmoidFn {
            midpoint: 0.5,
            steepness: 10.0,
        };

        // Sigmoid should be monotonically increasing
        let values = [0.0, 0.2, 0.4, 0.6, 0.8, 1.0];
        let results: Vec<f32> = values.iter().map(|&x| f.evaluate(x)).collect();

        for i in 1..results.len() {
            assert!(
                results[i] > results[i - 1],
                "Sigmoid should be monotonically increasing"
            );
        }
    }

    #[test]
    fn test_sigmoid_steepness() {
        let steep = SigmoidFn {
            midpoint: 0.5,
            steepness: 50.0,
        };
        let gentle = SigmoidFn {
            midpoint: 0.5,
            steepness: 2.0,
        };

        // Steeper function should have larger difference at extremes
        let steep_diff = steep.evaluate(0.8) - steep.evaluate(0.2);
        let gentle_diff = gentle.evaluate(0.8) - gentle.evaluate(0.2);

        assert!(
            steep_diff > gentle_diff,
            "Steeper function should have larger difference"
        );
    }

    #[test]
    fn test_get_sigmoid_config() {
        // Test default configuration
        let config = get_sigmoid_config("NONEXISTENT_ENV", 0.7, 15.0);
        assert!((config.midpoint - 0.7).abs() < 0.01);
        assert!((config.steepness - 15.0).abs() < 0.01);
    }

    #[test]
    fn test_sigmoid_edge_cases() {
        let f = SigmoidFn {
            midpoint: 0.5,
            steepness: 10.0,
        };

        // Test extreme values
        let very_low = f.evaluate(-1.0);
        let very_high = f.evaluate(2.0);

        assert!((0.0..=1.0).contains(&very_low));
        assert!((0.0..=1.0).contains(&very_high));
        assert!(very_low < very_high);
    }
}
