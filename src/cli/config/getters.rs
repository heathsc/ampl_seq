use std::path::PathBuf;

use super::Config;

impl Config {
    pub fn min_qual(&self) -> u8 {
        self.min_qual
    }
    
    pub fn output_prefix(&self) -> &str {
        self.output_prefix.as_ref()
    }   
   
   pub fn reference(&self) -> &[u8] {
       self.reference.as_ref()
   } 
   
   pub fn input_files(&self) -> &[PathBuf] {
       self.input_files.as_ref()
   }
   
   pub fn threads(&self) -> usize {
       self.threads
   }
   
   pub fn readers(&self) -> usize {
       self.readers
   }
}