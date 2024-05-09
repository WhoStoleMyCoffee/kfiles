use std::collections::HashMap;


fn is_char_sep(c: char) -> bool {
    !c.is_alphanumeric()
}




pub trait StringMatcher {
    fn set_query(&mut self, q: &str);

    fn score<S: AsRef<str>>(&self, target: &S) -> Option<isize>;

    fn matches<S: AsRef<str>>(&self, target: &S) -> bool {
        self.score(target).is_some()
    }
}




/// Simply checks whether `query` is contained within `target`
/// TODO choose between case insensitive search
#[derive(Debug, Clone)]
pub struct Contains(pub String);

impl StringMatcher for Contains {
    fn set_query(&mut self, q: &str) {
        self.0 = q.to_string();
    }

    fn score<S: AsRef<str>>(&self, target: &S) -> Option<isize> {
        if self.matches(target) {
            return Some(1);
        }
        None
    }

    fn matches<S: AsRef<str>>(&self, target: &S) -> bool {
        target.as_ref().to_lowercase() .contains(&self.0.to_lowercase())
    }
}



/// Naive method that checks whether the chars in `query` appear seuentially within `target`
/// TODO choose between case insensitive search
#[derive(Debug, Clone)]
pub struct Simple(pub String);

impl StringMatcher for Simple {
    fn set_query(&mut self, q: &str) {
        self.0 = q.to_string();
    }

    fn score<S: AsRef<str>>(&self, target: &S) -> Option<isize> {
        let mut score: usize = 100;

        let mut tchars = target.as_ref().chars();
        for qch in self.0.chars() {
            let qch = qch.to_lowercase().to_string();
            let p = tchars.position(|tch| tch.to_lowercase().to_string() == qch)?;
            score -= p;
        }

        Some(score as isize)
    }

    fn matches<S: AsRef<str>>(&self, target: &S) -> bool {
        let mut tchars = target.as_ref().chars();
        for qch in self.0.chars() {
            let qch = qch.to_lowercase().to_string();
            if !tchars.any(|tch| tch.to_lowercase().to_string() == qch) {
                return false;
            }
        }

        true
    }
}


// todo rename ig
struct Pick {
    index: usize,
    score: isize,
    consecutive_count: usize,
}


/// Method that imitates the string matching algorithm used in Sublime
#[derive(Debug, Clone)]
pub struct Sublime {
    pub bonus_consecutive: isize,
    pub bonus_word_start: isize,
    pub bonus_match_case: isize,
    pub penalty_distance: isize,
    query: Vec<char>,
}

impl Sublime {
    pub fn emphasize_word_starts() -> Self {
        Self::default()
    }

    pub fn emphasize_distance() -> Self {
        Sublime {
            bonus_consecutive: 12,
            bonus_word_start: 24,
            bonus_match_case:8,
            penalty_distance: 8,
            query: Vec::new(),
        }
    }

    pub fn with_query(mut self, query: &str) -> Self {
        self.query = query.chars().collect();
        self
    }

    fn score_recursive(
        &self,
        query_index: usize,
        target: &[char],
        target_index: usize,
        cache: &mut HashMap<(usize, usize), Option<(isize, usize)>>
    ) -> Option<Pick>
    {
        let Some(qch) = self.query.get(query_index) else {
            // Matched all query chars; success!
            return Some(Pick {
                index: target_index,
                score: 0,
                consecutive_count: 0,
            });
        };

        let qch_lower = qch.to_lowercase().to_string();

        // Get all occurences of qch
        // TODO
        // let occurences = target[target_index..].iter().enumerate()
        //     .filter(|(_, tch)| tch.to_lowercase().to_string() == qch_lower);
        let occurences = target.iter().enumerate()
            .skip(target_index)
            .filter(|(_, tch)| tch.to_lowercase().to_string() == qch_lower);

        let mut best: Option<Pick> = None;
        for (i, tch) in occurences {
            let this_key = (query_index, i);
            
            // If branch already computed
            if let Some(cached) = cache.get(&this_key) {
                // println!("-- qch = {qch}, ti = {target_index}");
                // println!("  cache hit!    {:?}", cached);

                // Update score
                // todo make dis better
                if let Some((score, this_consecutive)) = cached {
                    match best {
                        Some(b) if *score > b.score => best = Some(Pick {
                            index: i,
                            score: *score,
                            consecutive_count: *this_consecutive,
                        }),
                        None => best = Some(Pick {
                            index: i,
                            score: *score,
                            consecutive_count: *this_consecutive,
                        }),
                        _ => {},
                    }
                }

                continue;
            }
           

            // Branch out
            let Some(Pick {
                index: next_i,
                score: next_score,
                consecutive_count: next_consecutive
            }) = self.score_recursive(query_index + 1,target,i + 1,cache) else {
                // No match found in this branch
                continue;
            };

            // COMPUTE SCORE
            let dist = next_i - i - 1; // Chars in between
            let do_cases_match: bool = qch == tch;
            let this_consecutive: usize = if dist == 0 { next_consecutive + 1 } else { 0 };
            let is_start: bool = !is_char_sep(*tch) && if i == 0 {
                true
                // tch.is_uppercase() // todo remove if things look good
            } else {
                let prev_tch: char = target[i - 1]; // Will not fail
                is_char_sep(prev_tch) || (prev_tch.is_lowercase() && tch.is_uppercase())
            };

            let score: isize = next_score
                + (do_cases_match as isize) * self.bonus_match_case
                + (this_consecutive as isize) * self.bonus_consecutive
                // + self.bonus_consecutive.pow( this_consecutive as u32 ) // hmm...
                + (is_start as isize) * self.bonus_word_start
                - (dist as isize) * self.penalty_distance;
            // println!("-- qch = {qch}, tch = {tch} @ {i} : score = {}", score);
            // dbg!(next_i, do_cases_match, this_consecutive, is_start, dist);

            // Update score
            match best {
                Some(b) if score > b.score => best = Some(Pick {
                    index: i,
                    score,
                    consecutive_count: this_consecutive,
                }),
                None => best = Some(Pick {
                    index: i,
                    score,
                    consecutive_count: this_consecutive,
                }),
                _ => (),
            }
        }

        if let Some(Pick {
            index,
            score,
            consecutive_count
        }) = best {
            cache.insert((query_index, index), Some((score, consecutive_count)));
        }
        best
    }
}

impl StringMatcher for Sublime {
    fn set_query(&mut self, q: &str) {
        self.query = q.chars().collect();
    }

    fn score<S: AsRef<str>>(&self, target: &S) -> Option<isize> {
        let mut cache = HashMap::new();
        let t: Vec<char> = target.as_ref().chars().collect();
        self.score_recursive(0, &t, 0, &mut cache)
            .map(|Pick { score, .. }| score)
    }

    // Just check if chars in query appear sequentially within target
    fn matches<S: AsRef<str>>(&self, target: &S) -> bool {
        let mut tchars = target.as_ref().chars();
        for qch in self.query.iter() {
            let qch = qch.to_lowercase().to_string();
            if !tchars.any(|tch| tch.to_lowercase().to_string() == qch) {
                return false;
            }
        }

        true
    }
}

impl Default for Sublime {
    fn default() -> Self {
        Sublime {
            bonus_consecutive: 12,
            bonus_word_start: 72,
            bonus_match_case: 8,
            penalty_distance: 4,
            query: Vec::new(),
        }
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contains() {
        let targets: Vec<(&str, Option<isize>)> = vec![
            ("aBc", Some(1)),
            ("abc", Some(1)),
            ("cBa", None),
        ];

        let matcher = Contains("aBc".to_string());

        for (target, expected) in targets.iter() {
            let score = matcher.score(target);
            assert_eq!(score, *expected);
        }
    }

    #[test]
    fn simple() {
        let targets: Vec<(&str, Option<isize>)> = vec![
            ("aBc", Some(100)),
            ("abc", Some(100)),
            ("axbxc", Some(98)),
            ("cBa", None),
        ];

        let matcher = Simple("aBc".to_string());

        for (target, expected) in targets.iter() {
            let score = matcher.score(target);
            assert_eq!(score, *expected);
        }
    }

    #[test]
    fn sublime() {
        let matcher = Sublime::default()
            .with_query("abc");

        let Sublime {
            bonus_consecutive: sc,
            bonus_word_start: ss,
            bonus_match_case: sm,
            penalty_distance: pd,
            ..
        } = &matcher;
        dbg!(sc, ss, sm, pd);

        let targets: Vec<(&str, Option<isize>)> = vec![
            ("abc", Some( ss+sm+sc*3 + sm+sc*2 + sm+sc )),
            ("axbxc", Some( ss+sm + sm + sm+sc - pd*2 )),
            ("a b c", Some( ss+sm + ss+sm + ss+sm+sc - pd*2 )),
            ("abx", None),
            ("xa bo obo c", Some( sm + ss+sm + ss+sm+sc - pd*7 )),
        ];

        for (index, (target, expected)) in targets.iter().enumerate() {
            let score = matcher.score(target);
            assert_eq!(score, *expected, "index = {index}");
        }

    }
}

