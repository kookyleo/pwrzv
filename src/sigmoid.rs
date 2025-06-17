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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sigmoid_calculation() {
        let f = SigmoidFn {
            midpoint: 0.7,
            steepness: 8.0,
        };

        // 测试边界值
        assert!(f.evaluate(0.0) < 0.1);
        assert!(f.evaluate(1.0) > 0.9);
        assert!((f.evaluate(0.7) - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_sigmoid_mathematical_properties() {
        let f = SigmoidFn {
            midpoint: 0.5,
            steepness: 10.0,
        };

        // Test midpoint property: f(midpoint) ≈ 0.5
        let midpoint_result = f.evaluate(0.5);
        assert!((midpoint_result - 0.5).abs() < 0.001);

        // Test monotonicity: f(a) < f(b) when a < b
        assert!(f.evaluate(0.0) < f.evaluate(0.3));
        assert!(f.evaluate(0.3) < f.evaluate(0.5));
        assert!(f.evaluate(0.5) < f.evaluate(0.7));
        assert!(f.evaluate(0.7) < f.evaluate(1.0));

        // Test range: output should be in [0, 1]
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            let result = f.evaluate(x);
            assert!(
                (0.0..=1.0).contains(&result),
                "f({x}) = {result} not in [0,1]"
            );
        }
    }

    #[test]
    fn test_sigmoid_steepness_effect() {
        let gentle = SigmoidFn {
            midpoint: 0.5,
            steepness: 2.0,
        };

        let steep = SigmoidFn {
            midpoint: 0.5,
            steepness: 20.0,
        };

        // At the midpoint, both should be ~0.5
        assert!((gentle.evaluate(0.5) - 0.5).abs() < 0.01);
        assert!((steep.evaluate(0.5) - 0.5).abs() < 0.01);

        // Away from midpoint, steep should change more dramatically
        let gentle_low = gentle.evaluate(0.3);
        let steep_low = steep.evaluate(0.3);

        // Steep curve should have lower value at 0.3 (further from 0.5)
        assert!(steep_low < gentle_low);

        let gentle_high = gentle.evaluate(0.7);
        let steep_high = steep.evaluate(0.7);

        // Steep curve should have higher value at 0.7 (closer to 1.0)
        assert!(steep_high > gentle_high);
    }

    #[test]
    fn test_sigmoid_midpoint_effect() {
        let early = SigmoidFn {
            midpoint: 0.3,
            steepness: 8.0,
        };

        let late = SigmoidFn {
            midpoint: 0.7,
            steepness: 8.0,
        };

        // Early midpoint should reach 0.5 at x=0.3
        assert!((early.evaluate(0.3) - 0.5).abs() < 0.01);

        // Late midpoint should reach 0.5 at x=0.7
        assert!((late.evaluate(0.7) - 0.5).abs() < 0.01);

        // At x=0.5, early should be higher than late
        assert!(early.evaluate(0.5) > late.evaluate(0.5));
    }

    #[test]
    fn test_sigmoid_extreme_values() {
        let f = SigmoidFn {
            midpoint: 0.5,
            steepness: 10.0,
        };

        // Test very negative input
        let very_low = f.evaluate(-10.0);
        assert!(very_low < 0.01);

        // Test very positive input
        let very_high = f.evaluate(10.0);
        assert!(very_high > 0.99);

        // Test exactly at zero
        let at_zero = f.evaluate(0.0);
        assert!(at_zero > 0.0 && at_zero < 1.0);

        // Test NaN protection (sigmoid should handle any finite input)
        assert!(f.evaluate(f32::MAX).is_finite());
        assert!(f.evaluate(f32::MIN).is_finite());
    }

    #[test]
    fn test_sigmoid_zero_steepness() {
        let f = SigmoidFn {
            midpoint: 0.5,
            steepness: 0.0,
        };

        // With zero steepness, should always return 0.5
        assert!((f.evaluate(0.0) - 0.5).abs() < 0.01);
        assert!((f.evaluate(0.5) - 0.5).abs() < 0.01);
        assert!((f.evaluate(1.0) - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_sigmoid_high_steepness() {
        let f = SigmoidFn {
            midpoint: 0.5,
            steepness: 100.0,
        };

        // With high steepness, should be step-like
        assert!(f.evaluate(0.4) < 0.1); // Well below midpoint
        assert!(f.evaluate(0.6) > 0.9); // Well above midpoint
        assert!((f.evaluate(0.5) - 0.5).abs() < 0.01); // At midpoint
    }

    #[test]
    fn test_sigmoid_default_values() {
        let default_fn = SigmoidFn::default();
        assert_eq!(default_fn.midpoint, 0.5);
        assert_eq!(default_fn.steepness, 8.0);

        // Default should have reasonable behavior
        assert!(default_fn.evaluate(0.0) < 0.2);
        assert!(default_fn.evaluate(1.0) > 0.8);
        assert!((default_fn.evaluate(0.5) - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_sigmoid_configuration_ranges() {
        // Test common configuration ranges used in the application

        // CPU usage (midpoint ~0.6-0.7, steepness ~8)
        let cpu_config = SigmoidFn {
            midpoint: 0.65,
            steepness: 8.0,
        };
        assert!(cpu_config.evaluate(0.3) < cpu_config.evaluate(0.8));

        // Memory pressure (midpoint ~0.3, steepness ~12)
        let memory_config = SigmoidFn {
            midpoint: 0.3,
            steepness: 12.0,
        };
        assert!((memory_config.evaluate(0.3) - 0.5).abs() < 0.05);

        // Network packet loss (midpoint ~0.01, steepness ~50)
        let network_config = SigmoidFn {
            midpoint: 0.01,
            steepness: 50.0,
        };
        assert!(network_config.evaluate(0.005) < 0.5);
        assert!(network_config.evaluate(0.02) > 0.5);
    }

    #[test]
    fn test_sigmoid_precision() {
        let f = SigmoidFn {
            midpoint: 0.5,
            steepness: 8.0,
        };

        // Test that small changes in input produce appropriate changes in output
        let base = f.evaluate(0.5);
        let slightly_higher = f.evaluate(0.501);
        let slightly_lower = f.evaluate(0.499);

        assert!(slightly_lower < base);
        assert!(base < slightly_higher);

        // The differences should be small but detectable
        assert!((slightly_higher - base) > 0.0);
        assert!((base - slightly_lower) > 0.0);
    }

    #[test]
    fn test_sigmoid_debug_and_clone() {
        let f = SigmoidFn {
            midpoint: 0.6,
            steepness: 10.0,
        };

        // Test Debug trait
        let debug_str = format!("{f:?}");
        assert!(debug_str.contains("0.6"));
        assert!(debug_str.contains("10"));

        // Test Clone trait
        let f_clone = f;
        assert_eq!(f.midpoint, f_clone.midpoint);
        assert_eq!(f.steepness, f_clone.steepness);
        assert_eq!(f.evaluate(0.5), f_clone.evaluate(0.5));

        // Test Copy trait
        let f_copy = f;
        assert_eq!(f.evaluate(0.5), f_copy.evaluate(0.5));
    }
}
