//! Dart 3.6.2 List.sort for root path lengths, including equal-key movement.
//!
//! Adapted from sdk/lib/internal/sort.dart (Copyright 2011, the Dart project
//! authors). BSD license and attribution: THIRD_PARTY_NOTICES.md.

/// Sort indices by cached UTF-16 lengths. Rust's stable sort and unstable
/// sort both differ from Dart's dual-pivot sort on large equal-key groups.
pub(super) fn sort(a: &mut [usize], lengths: &[usize]) {
    if a.len() <= 33 {
        for i in 1..a.len() {
            let element = a[i];
            let mut j = i;
            while j > 0 && lengths[a[j - 1]] > lengths[element] {
                a[j] = a[j - 1];
                j -= 1;
            }
            a[j] = element;
        }
        return;
    }
    let right = a.len() - 1;
    let sixth = a.len() / 6;
    let i1 = sixth;
    let i3 = right / 2;
    let i2 = i3 - sixth;
    let i4 = i3 + sixth;
    let i5 = right - sixth;
    let mut sample = [a[i1], a[i2], a[i3], a[i4], a[i5]];
    for (x, y) in [
        (0, 1),
        (3, 4),
        (0, 2),
        (1, 2),
        (0, 3),
        (2, 3),
        (1, 4),
        (1, 2),
        (3, 4),
    ] {
        if lengths[sample[x]] > lengths[sample[y]] {
            sample.swap(x, y);
        }
    }
    let pivot1 = sample[1];
    let pivot2 = sample[3];
    a[i1] = sample[0];
    a[i3] = sample[2];
    a[i5] = sample[4];
    a[i2] = a[0];
    a[i4] = a[right];
    let mut less = 1;
    let mut great = right - 1;
    let equal = lengths[pivot1] == lengths[pivot2];
    let mut k = less;
    while k <= great {
        let element = a[k];
        if equal {
            match lengths[element].cmp(&lengths[pivot1]) {
                std::cmp::Ordering::Equal => {}
                std::cmp::Ordering::Less => {
                    a.swap(k, less);
                    less += 1;
                }
                std::cmp::Ordering::Greater => loop {
                    match lengths[a[great]].cmp(&lengths[pivot1]) {
                        std::cmp::Ordering::Greater => great -= 1,
                        std::cmp::Ordering::Less => {
                            a[k] = a[less];
                            a[less] = a[great];
                            less += 1;
                            a[great] = element;
                            great -= 1;
                            break;
                        }
                        std::cmp::Ordering::Equal => {
                            a[k] = a[great];
                            a[great] = element;
                            great -= 1;
                            break;
                        }
                    }
                },
            }
        } else if lengths[element] < lengths[pivot1] {
            a.swap(k, less);
            less += 1;
        } else if lengths[element] > lengths[pivot2] {
            loop {
                if lengths[a[great]] > lengths[pivot2] {
                    great -= 1;
                    if great < k {
                        break;
                    }
                } else {
                    if lengths[a[great]] < lengths[pivot1] {
                        a[k] = a[less];
                        a[less] = a[great];
                        less += 1;
                    } else {
                        a[k] = a[great];
                    }
                    a[great] = element;
                    great -= 1;
                    break;
                }
            }
        }
        k += 1;
    }
    a[0] = a[less - 1];
    a[less - 1] = pivot1;
    a[right] = a[great + 1];
    a[great + 1] = pivot2;
    sort(&mut a[..less - 1], lengths);
    sort(&mut a[great + 2..], lengths);
    if equal {
        return;
    }
    if less < i1 && great > i5 {
        while lengths[a[less]] == lengths[pivot1] {
            less += 1;
        }
        while lengths[a[great]] == lengths[pivot2] {
            great -= 1;
        }
        k = less;
        while k <= great {
            let element = a[k];
            if lengths[element] == lengths[pivot1] {
                a.swap(k, less);
                less += 1;
            } else if lengths[element] == lengths[pivot2] {
                loop {
                    if lengths[a[great]] == lengths[pivot2] {
                        great -= 1;
                        if great < k {
                            break;
                        }
                    } else {
                        if lengths[a[great]] < lengths[pivot1] {
                            a[k] = a[less];
                            a[less] = a[great];
                            less += 1;
                        } else {
                            a[k] = a[great];
                        }
                        a[great] = element;
                        great -= 1;
                        break;
                    }
                }
            }
            k += 1;
        }
    }
    if less <= great {
        sort(&mut a[less..=great], lengths);
    }
}
