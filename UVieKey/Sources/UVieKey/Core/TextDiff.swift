import Foundation

/// Computes the text diff between old and new composing strings.
/// Returns the number of backspaces needed and the new suffix to type.
struct TextDiff {
    let backspaceCount: Int
    let newSuffix: String

    init(old: String, new: String) {
        let oldChars = Array(old)
        let newChars = Array(new)

        // Find common prefix
        var commonPrefix = 0
        let minLen = min(oldChars.count, newChars.count)
        while commonPrefix < minLen && oldChars[commonPrefix] == newChars[commonPrefix] {
            commonPrefix += 1
        }

        self.backspaceCount = oldChars.count - commonPrefix
        self.newSuffix = String(newChars[commonPrefix...])
    }
}
