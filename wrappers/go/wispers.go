package wispersgo

/*
#include "wispers_helpers.h"
#include <stdlib.h>
*/
import "C"
import "unsafe"

// NodeState represents the state of a wispers node.
type NodeState int

const (
	NodeStatePending    NodeState = 0
	NodeStateRegistered NodeState = 1
	NodeStateActivated  NodeState = 2
)

func (s NodeState) String() string {
	switch s {
	case NodeStatePending:
		return "Pending"
	case NodeStateRegistered:
		return "Registered"
	case NodeStateActivated:
		return "Activated"
	default:
		return "Unknown"
	}
}

// ActivationStatus represents the activation status of a node in the group.
type ActivationStatus int32

const (
	ActivationUnknown      ActivationStatus = 0
	ActivationNotActivated ActivationStatus = 1
	ActivationActivated    ActivationStatus = 2
)

// ActivationAction represents what action the calling node should take.
type ActivationAction int32

const (
	ActivationActionAlone          ActivationAction = 0
	ActivationActionBootstrap      ActivationAction = 1
	ActivationActionNeedActivation ActivationAction = 2
	ActivationActionCanEndorse     ActivationAction = 3
	ActivationActionAllActivated   ActivationAction = 4
)

func (a ActivationAction) String() string {
	switch a {
	case ActivationActionAlone:
		return "Alone"
	case ActivationActionBootstrap:
		return "Bootstrap"
	case ActivationActionNeedActivation:
		return "NeedActivation"
	case ActivationActionCanEndorse:
		return "CanEndorse"
	case ActivationActionAllActivated:
		return "AllActivated"
	default:
		return "Unknown"
	}
}

// NodeInfo contains information about a node in the connectivity group.
type NodeInfo struct {
	NodeNumber       int32
	Name             string
	IsSelf           bool
	ActivationStatus ActivationStatus
	LastSeenAtMillis int64
	IsOnline         bool
}

// GroupStatus is a snapshot of the connectivity group's activation state.
type GroupStatus struct {
	Action ActivationAction
	Nodes  []NodeInfo
}

// RegistrationInfo contains registration information for a node.
type RegistrationInfo struct {
	ConnectivityGroupID string
	NodeNumber          int32
	AuthToken           string
	AttestationJWT      string // Signed JWT attesting to (cg_id, node_number)
}

// initResult is the internal type sent through the bridge channel for RestoreOrInit.
type initResult struct {
	nodePtr unsafe.Pointer
	state   NodeState
}

// startServingResult is the internal type sent through the bridge channel for StartServing.
type startServingResult struct {
	servingPtr  unsafe.Pointer
	sessionPtr  unsafe.Pointer
	incomingPtr unsafe.Pointer
}

// groupStatusResult is the internal type sent through the bridge channel for GroupStatus.
type groupStatusResult struct {
	action ActivationAction
	nodes  []NodeInfo
}

// dataResult is the internal type sent through the bridge channel for data callbacks.
type dataResult struct {
	data []byte
}
