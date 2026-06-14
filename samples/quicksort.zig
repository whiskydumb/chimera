const std = @import("std");

fn partition(comptime T: type, items: []T) usize {
    const pivot = items[items.len - 1];
    var i: usize = 0;
    var j: usize = 0;
    while (j < items.len - 1) : (j += 1) {
        if (items[j] < pivot) {
            std.mem.swap(T, &items[i], &items[j]);
            i += 1;
        }
    }
    std.mem.swap(T, &items[i], &items[items.len - 1]);
    return i;
}

pub fn quicksort(comptime T: type, items: []T) void {
    if (items.len <= 1) return;
    const p = partition(T, items);
    quicksort(T, items[0..p]);
    quicksort(T, items[p + 1 ..]);
}

test "sorts ascending" {
    var data = [_]i32{ 5, 2, 9, 1, 3 };
    quicksort(i32, &data);
    try std.testing.expectEqualSlices(i32, &[_]i32{ 1, 2, 3, 5, 9 }, &data);
}
