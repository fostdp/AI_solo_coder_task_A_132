use crate::models::{SensorData, ClepsydraConfig};

pub struct HydraulicModel {
    gravity: f64,
    standard_pressure: f64,
}

impl HydraulicModel {
    pub fn new() -> Self {
        Self {
            gravity: 980.665,
            standard_pressure: 101.325,
        }
    }

    pub fn pressure_correction(&self, pressure_kpa: f64) -> f64 {
        let p = pressure_kpa.max(50.0).min(110.0);
        (self.standard_pressure / p).powf(0.5)
    }

    pub fn altitude_to_pressure(&self, altitude_m: f64) -> f64 {
        self.standard_pressure * (1.0 - 2.25577e-5 * altitude_m).powf(5.25588)
    }

    pub fn calculate_theoretical_flow(
        &self,
        water_level: f64,
        config: &ClepsydraConfig,
        water_temp: f64,
    ) -> f64 {
        let head = water_level / 10.0;
        let velocity = (2.0 * self.gravity * head).sqrt();
        let orifice_area = std::f64::consts::PI * (config.orifice_diameter / 2.0).powi(2);
        let base_flow = config.flow_coefficient * orifice_area * velocity;
        let viscosity_factor = self.viscosity_correction(water_temp);
        base_flow * viscosity_factor
    }

    fn viscosity_correction(&self, temp_c: f64) -> f64 {
        let t = temp_c.max(0.0).min(100.0);
        let nu = 1.792e-2 / (1.0 + 0.0337 * t + 0.000221 * t.powi(2));
        let nu_ref = 1.308e-2;
        (nu_ref / nu).powf(0.1)
    }

    pub fn calculate_evaporation_rate(
        &self,
        water_temp: f64,
        humidity: f64,
        surface_area: f64,
        quality: f64,
        pressure_kpa: f64,
    ) -> f64 {
        let t_kelvin = water_temp + 273.15;
        let svp = 610.78 * ((17.27 * water_temp) / (water_temp + 237.3)).exp();
        let avp = svp * (humidity / 100.0);
        let pressure_diff = svp - avp;
        let pressure_factor = self.pressure_correction(pressure_kpa);
        let mass_flux = 0.001 * pressure_diff / t_kelvin.sqrt() * pressure_factor;
        let volume_flux = mass_flux * surface_area * quality / 1000.0;
        volume_flux
    }

    pub fn calculate_flow_error(
        &self,
        theoretical: f64,
        actual: f64,
    ) -> f64 {
        if theoretical.abs() < 1e-9 {
            return 0.0;
        }
        ((actual - theoretical) / theoretical) * 100.0
    }

    pub fn update_daily_error(
        &self,
        current_error: f64,
        flow_error_rate: f64,
        dt_seconds: f64,
    ) -> f64 {
        let error_accumulation = flow_error_rate / 100.0 * dt_seconds;
        current_error + error_accumulation
    }

    pub fn simulate_water_level(
        &self,
        sensor: &SensorData,
        config: &ClepsydraConfig,
        inflow: f64,
        dt: f64,
    ) -> f64 {
        let net_flow = inflow - sensor.flow_rate;
        let volume_change = net_flow * dt;
        let level_change = volume_change / config.cross_section_area;
        (sensor.water_level + level_change).clamp(config.min_level, config.max_level)
    }

    pub fn orifice_flow_from_level(
        &self,
        level: f64,
        config: &ClepsydraConfig,
        temp: f64,
    ) -> f64 {
        let head = level / 10.0;
        let orifice_area = std::f64::consts::PI * (config.orifice_diameter / 2.0).powi(2);
        let velocity = (2.0 * self.gravity * head).sqrt();
        let viscosity_factor = self.viscosity_correction(temp);
        config.flow_coefficient * orifice_area * velocity * viscosity_factor
    }
}

impl Default for HydraulicModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ClepsydraConfig {
        ClepsydraConfig {
            clepsydra_id: "KD1".to_string(),
            name: "天上壶".to_string(),
            max_level: 120.0,
            min_level: 20.0,
            standard_flow: 2.5,
            cross_section_area: 78.54,
            orifice_diameter: 0.3,
            flow_coefficient: 0.62,
        }
    }

    #[test]
    fn test_theoretical_flow() {
        let model = HydraulicModel::new();
        let config = test_config();
        let flow = model.calculate_theoretical_flow(100.0, &config, 20.0);
        assert!(flow > 0.0);
        assert!(flow < 10.0);
    }

    #[test]
    fn test_evaporation() {
        let model = HydraulicModel::new();
        let evap = model.calculate_evaporation_rate(25.0, 60.0, 100.0, 1.0, 101.325);
        assert!(evap >= 0.0);
    }

    #[test]
    fn test_pressure_correction() {
        let model = HydraulicModel::new();
        let sea_level = model.pressure_correction(101.325);
        let high_altitude = model.pressure_correction(70.0);
        assert!((sea_level - 1.0).abs() < 0.01);
        assert!(high_altitude > 1.0);
    }

    #[test]
    fn test_viscosity_correction() {
        let model = HydraulicModel::new();
        let corr = model.viscosity_correction(20.0);
        assert!((corr - 1.0).abs() < 0.2);
    }
}
