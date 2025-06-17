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
}
