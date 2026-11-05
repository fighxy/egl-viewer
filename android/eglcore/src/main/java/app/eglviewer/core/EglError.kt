package app.eglviewer.core

/**
 * A failure reported by the native core, with its reason (mirrors Swift `EglError`, D-092).
 */
class EglError(
    val code: Code,
    override val message: String,
) : Exception(if (message.isEmpty()) code.name else message) {

    enum class Code(val raw: Int) {
        INVALID(1),
        NOT_FOUND(2),
        READ_ONLY(3),
        NOT_A_LIBRARY(4),
        STORAGE(5),
        UNSUPPORTED(6),
        OTHER(7),
        WRONG_PASSWORD(8),
        LOCKED(9),
        ;

        companion object {
            fun fromRaw(status: Int): Code =
                entries.firstOrNull { it.raw == status } ?: OTHER
        }
    }

    companion object {
        fun of(status: Int, message: String): EglError =
            EglError(Code.fromRaw(status), message)
    }
}
