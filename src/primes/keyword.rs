use std::collections::HashMap;
use std::sync::RwLock;

use std::sync::atomic::{AtomicUsize, Ordering};

static KEYWORD_ID: AtomicUsize = AtomicUsize::new(0);

lazy_static! {
  /// use 2 maps for fast lookups
  static ref KEYWORDS_DICT: RwLock<HashMap<String, usize>> = RwLock::new(HashMap::new());
  static ref KEYWORDS_REVERSE_DICT: RwLock<HashMap<usize, String>> = RwLock::new(HashMap::new());
}

/// lookup from maps, record new keywords
pub fn load_order_key(s: &str) -> usize {
  let mut ret: usize = 0;
  let existed = {
    let read_dict = KEYWORDS_DICT.read().unwrap();
    if read_dict.contains_key(s) {
      ret = read_dict[s].to_owned();
      true
    } else {
      false
    }
  };
  // boring logic to make sure reading lock released
  if !existed {
    let mut dict = KEYWORDS_DICT.write().unwrap();
    let mut reverse_dict = KEYWORDS_REVERSE_DICT.write().unwrap();
    ret = KEYWORD_ID.fetch_add(1, Ordering::SeqCst);

    (*dict).insert(s.to_owned(), ret);
    (*reverse_dict).insert(ret, s.to_owned());
  }
  ret
}

pub fn lookup_order_kwd_str(i: &usize) -> String {
  let reverse_dict = KEYWORDS_REVERSE_DICT.read().unwrap();
  reverse_dict[i].to_owned()
}
