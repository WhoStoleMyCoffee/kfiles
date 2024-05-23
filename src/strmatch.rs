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
/// Returns `Some(`[`DEFAULT_SCORE`]`)` if matches
#[derive(Debug, Clone)]
pub struct Contains {
    pub query: String,
    case_insensitive: bool,
}

impl Contains {
    const DEFAULT_SCORE: isize = 100;

    pub fn new(query: String) -> Self {
        Contains {
            query,
            case_insensitive: false,
        }
    }

    pub fn case_insensitive(mut self) -> Self {
        self.case_insensitive = true;
        self
    }
}

impl StringMatcher for Contains {
    fn set_query(&mut self, q: &str) {
        self.query = q.to_string();
    }

    fn score<S: AsRef<str>>(&self, target: &S) -> Option<isize> {
        if self.matches(target) {
            return Some(Contains::DEFAULT_SCORE);
        }
        None
    }

    fn matches<S: AsRef<str>>(&self, target: &S) -> bool {
        if self.case_insensitive {
            target.as_ref().to_lowercase() .contains(&self.query.to_lowercase())
        } else {
            target.as_ref() .contains(&self.query)
        }
    }
}



/// Naive method that checks whether the chars in `query` appear seuentially within `target`
/// The returned score will have a maximum of `0`, subtracting 1 for every character in between
/// matches
#[derive(Debug, Clone)]
pub struct Simple {
    pub query: String,
    case_insensitive: bool,
}

impl Simple {
    pub fn new(query: String) -> Self {
        Simple {
            query,
            case_insensitive: false,
        }
    }

    pub fn case_insensitive(mut self) -> Self {
        self.case_insensitive = true;
        self
    }
}

impl StringMatcher for Simple {
    fn set_query(&mut self, q: &str) {
        self.query = q.to_string();
    }

    fn score<S: AsRef<str>>(&self, target: &S) -> Option<isize> {
        let mut tchars = target.as_ref().chars();

        if self.case_insensitive {
            let mut score: isize = 0;
            for qch in self.query.chars() {
                let qch = qch.to_lowercase().to_string();
                let p = tchars.position(|tch| tch.to_lowercase().to_string() == qch)?;
                score -= p as isize;
            }
            return Some(score);
        }

        let mut score: isize = 0;
        for qch in self.query.chars() {
            let p = tchars.position(|tch| tch == qch)?;
            score -= p as isize;
        }
        Some(score)
    }

    fn matches<S: AsRef<str>>(&self, target: &S) -> bool {
        let mut tchars = target.as_ref().chars();

        // bruh
        if self.case_insensitive {
            for qch in self.query.chars() {
                let qch = qch.to_lowercase().to_string();
                if !tchars.any(|tch| tch.to_lowercase().to_string() == qch) {
                    return false;
                }
            }
            return true;
        }

        for qch in self.query.chars() {
            if !tchars.any(|tch| tch == qch) {
                return false;
            }
        }
        true
    }
}


struct Match {
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
    ) -> Option<Match>
    {
        let Some(qch) = self.query.get(query_index) else {
            // Matched all query chars; success!
            return Some(Match {
                index: target_index,
                score: 0,
                consecutive_count: 0,
            });
        };

        let qch_lower = qch.to_lowercase().to_string();

        // Get all occurences of qch
        let occurences = target[target_index..].iter().enumerate()
            .filter(|(_, ch)| ch.to_lowercase().to_string() == qch_lower)
            .map(|(i, ch)| (i + target_index, ch));

        let mut best: Option<Match> = None;
        for (i, tch) in occurences {
            let this_key = (query_index, i);
            
            // If branch already computed
            if let Some(cached) = cache.get(&this_key) {
                // println!("-- qch = {qch}, ti = {target_index}");
                // println!("  cache hit!    {:?}", cached);

                let Some((score, this_consecutive)) = cached else {
                    // There was no match in this branch
                    continue;
                };

                match best {
                    Some(ref b) if *score <= b.score => continue,
                    _ => {},
                }

                // We have a new best
                best = Some(Match {
                    index: i,
                    score: *score,
                    consecutive_count: *this_consecutive,
                });

                continue;
            }
           

            // Branch out
            let Some(Match {
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
                Some(b) if score > b.score => best = Some(Match {
                    index: i,
                    score,
                    consecutive_count: this_consecutive,
                }),
                None => best = Some(Match {
                    index: i,
                    score,
                    consecutive_count: this_consecutive,
                }),
                _ => (),
            }
        }

        if let Some(Match {
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
        if self.query.is_empty() {
            return Some(0);
        }

        let mut cache = HashMap::new();
        let t: Vec<char> = target.as_ref().chars().collect();
        self.score_recursive(0, &t, 0, &mut cache)
            .map(|Match { score, .. }| score)
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
        let s = Contains::DEFAULT_SCORE;
        let targets: Vec<(&str, Option<isize>)> = vec![
            ("aBc", Some(s)),
            ("abc", Some(s)),
            ("cBa", None),
            ("", None),
        ];

        let matcher = Contains::new("aBc".to_string())
            .case_insensitive();

        for (target, expected) in targets.iter() {
            let score = matcher.score(target);
            assert_eq!(score, *expected);
        }
    }

    #[test]
    fn simple() {
        let targets: Vec<(&str, Option<isize>)> = vec![
            ("aBc", Some(0)),
            ("abc", Some(0)),
            ("axbxc", Some(-2)),
            ("cBa", None),
            ("", None),
        ];

        let matcher = Simple::new("aBc".to_string())
            .case_insensitive();

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
            ("", None),
        ];

        for (index, (target, expected)) in targets.iter().enumerate() {
            let score = matcher.score(target);
            assert_eq!(score, *expected, "index = {index}");
        }

    }
}

