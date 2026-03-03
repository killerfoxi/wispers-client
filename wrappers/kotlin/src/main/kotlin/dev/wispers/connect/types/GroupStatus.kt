package dev.wispers.connect.types

/**
 * What action the calling node should take regarding activation.
 */
sealed class ActivationAction(val code: Int) {
    /** Only node in the group — nothing to activate with. */
    data object Alone : ActivationAction(0)

    /** No activated nodes (empty or dead roster). Any node can pair with any other. */
    data object Bootstrap : ActivationAction(1)

    /** Roster exists with activated peers — this node needs a code from one. */
    data object NeedActivation : ActivationAction(2)

    /** This node is activated; unactivated peers exist that can be endorsed. */
    data object CanEndorse : ActivationAction(3)

    /** All nodes in the group are activated. */
    data object AllActivated : ActivationAction(4)

    companion object {
        fun fromCode(code: Int): ActivationAction = when (code) {
            0 -> Alone
            1 -> Bootstrap
            2 -> NeedActivation
            3 -> CanEndorse
            4 -> AllActivated
            else -> Alone
        }
    }
}

/**
 * Snapshot of the connectivity group's activation state.
 */
data class GroupStatus(
    /** What action the calling node should take. */
    val action: ActivationAction,

    /** All nodes in the connectivity group. */
    val nodes: List<NodeInfo>
)
