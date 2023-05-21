use std::cmp::min;
use unicode_segmentation::UnicodeSegmentation;

pub fn levenshtein_distance(left: &str, right: &str) -> usize {
    let l: Vec<&str> = left.graphemes(true).collect::<Vec<&str>>();
    let r: Vec<&str> = right.graphemes(true).collect::<Vec<&str>>();
    let rows = r.len() + 1;
    let cols = l.len() + 1;
    let mut dp_table = vec![vec![0; cols]; rows];

    // Init the first row and the first col of the dp_table
    for i in 0..rows {
        dp_table[i][0] = i;
    }

    for i in 0..cols {
        dp_table[0][i] = i;
    }

    // walk through
    for row in 1..rows {
        for col in 1..cols {
            if l[col - 1] == r[row - 1] {
                dp_table[row][col] = dp_table[row - 1][col - 1];
            } else {
                dp_table[row][col] = min(
                    dp_table[row - 1][col - 1],
                    min(dp_table[row - 1][col], dp_table[row][col - 1]),
                ) + 1;
            }
        }
    }

    return dp_table[rows - 1][cols - 1];
}