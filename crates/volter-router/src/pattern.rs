//! [`RoutePattern`] — a path pattern with named parameters (e.g. `/users/:id`).
//!
//! This is a simple segment-based matcher that will be replaced by a radix-tree
//! implementation in a later PR.  The public API of [`Router`](crate::Router)
//! is designed to remain compatible with that change.

/// A segment in a route pattern.
#[derive(Debug, Clone, PartialEq)]
enum Segment {
    /// A literal path segment (e.g. `"users"` in `/users/:id`).
    Static(String),
    /// A named parameter (e.g. `"id"` from `:id`).
    Param(String),
}

/// A parsed route pattern supporting named parameters.
///
/// # Examples
///
/// - `/users/:id` matches `/users/42` and captures `id = "42"`.
/// - `/posts/:post_id/comments/:comment_id` matches
///   `/posts/1/comments/2` and captures `post_id = "1"`,
///   `comment_id = "2"`.
/// - `/users/:id` does **not** match `/users` or `/users/42/extra`
///   (segment count must match exactly).
#[derive(Debug, Clone)]
pub(crate) struct RoutePattern {
    segments: Vec<Segment>,
}

impl RoutePattern {
    /// Parse a path pattern into a [`RoutePattern`].
    ///
    /// Segments starting with `:` are treated as named parameters; all
    /// others are treated as literal strings.
    ///
    /// An empty segment list (pattern `/` or empty string) produces a
    /// matcher that only matches the root path `/`.
    pub(crate) fn parse(pattern: &str) -> Self {
        let segments: Vec<Segment> = pattern
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| {
                if let Some(name) = s.strip_prefix(':') {
                    Segment::Param(name.to_owned())
                } else {
                    Segment::Static(s.to_owned())
                }
            })
            .collect();
        Self { segments }
    }

    /// Try to match a request path against this pattern.
    ///
    /// Returns the extracted parameter pairs if the path matches, or `None`
    /// if the segment count differs or a static segment does not match.
    pub(crate) fn matches(&self, path: &str) -> Option<Vec<(String, String)>> {
        let path_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

        if path_segments.len() != self.segments.len() {
            return None;
        }

        let mut params = Vec::new();
        for (segment, path_seg) in self.segments.iter().zip(path_segments.iter()) {
            match segment {
                Segment::Static(expected) => {
                    if expected != path_seg {
                        return None;
                    }
                }
                Segment::Param(name) => {
                    params.push((name.clone(), (*path_seg).to_string()));
                }
            }
        }

        Some(params)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_route_matches() {
        let pattern = RoutePattern::parse("/users");
        assert_eq!(pattern.matches("/users"), Some(vec![]));
    }

    #[test]
    fn static_route_does_not_match_different_path() {
        let pattern = RoutePattern::parse("/users");
        assert_eq!(pattern.matches("/posts"), None);
    }

    #[test]
    fn root_path_matches() {
        let pattern = RoutePattern::parse("/");
        assert_eq!(pattern.matches("/"), Some(vec![]));
    }

    #[test]
    fn single_param() {
        let pattern = RoutePattern::parse("/users/:id");
        assert_eq!(
            pattern.matches("/users/42"),
            Some(vec![("id".to_string(), "42".to_string())])
        );
    }

    #[test]
    fn multiple_params() {
        let pattern = RoutePattern::parse("/posts/:post_id/comments/:comment_id");
        let result = pattern.matches("/posts/1/comments/2");
        assert_eq!(
            result,
            Some(vec![
                ("post_id".to_string(), "1".to_string()),
                ("comment_id".to_string(), "2".to_string()),
            ])
        );
    }

    #[test]
    fn param_does_not_match_different_length() {
        let pattern = RoutePattern::parse("/users/:id");
        assert_eq!(pattern.matches("/users"), None);
        assert_eq!(pattern.matches("/users/42/extra"), None);
    }

    #[test]
    fn param_does_not_match_static_mismatch() {
        let pattern = RoutePattern::parse("/users/:id");
        assert_eq!(pattern.matches("/posts/42"), None);
    }

    #[test]
    fn string_param_value() {
        let pattern = RoutePattern::parse("/users/:name");
        assert_eq!(
            pattern.matches("/users/alice"),
            Some(vec![("name".to_string(), "alice".to_string())])
        );
    }
}
