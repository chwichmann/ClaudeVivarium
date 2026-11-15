//! Command line: flags (`--demo`) and options with a value (`--themes space`).
pub struct Args {
    list: Vec<String>,
}

impl Args {
    pub fn new(list: Vec<String>) -> Self {
        Args { list }
    }

    pub fn flag(&self, name: &str) -> bool {
        self.list.iter().any(|a| a == name)
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        let i = self.list.iter().position(|a| a == name)?;
        self.list.get(i + 1).map(String::as_str)
    }

    pub fn num(&self, name: &str, default: f64) -> Result<f64, String> {
        match self.value(name) {
            None => Ok(default),
            Some(v) => v.parse().map_err(|_| format!("{name}: '{v}' is not a number")),
        }
    }

    /// "1920x1200" -> (1920, 1200)
    pub fn size(&self, name: &str, default: (u32, u32)) -> Result<(u32, u32), String> {
        let Some(v) = self.value(name) else { return Ok(default) };
        let parts: Vec<u32> = v.split('x').filter_map(|p| p.trim().parse().ok()).collect();
        if parts.len() == 2 && parts[0] > 0 && parts[1] > 0 { Ok((parts[0], parts[1])) } else { Err(format!("{name}: expected WIDTHxHEIGHT, got '{v}'")) }
    }
}
