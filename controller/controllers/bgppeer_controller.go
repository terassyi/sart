/*
Copyright 2023.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

package controllers

import (
	"context"
	"fmt"
	"reflect"
	"time"

	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/event"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/predicate"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/constants"
	"github.com/terassyi/sart/controller/pkg/speaker"
)

// BGPPeerReconciler reconciles a BGPPeer object
type BGPPeerReconciler struct {
	client.Client
	Scheme              *runtime.Scheme
	SpeakerEndpointPort uint32
	SpeakerType         speaker.SpeakerType
}

//+kubebuilder:rbac:groups=sart.terassyi.net,resources=bgppeers,verbs=get;list;watch;create;update;patch;delete
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=bgppeers/status,verbs=get;update;patch
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=bgppeers/finalizers,verbs=update
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=nodebgps,verbs=get;list;update;patch
//+kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements,verbs=watch;get;list;update;patch

// Reconcile is part of the main kubernetes reconciliation loop which aims to
// move the current state of the cluster closer to the desired state.
// TODO(user): Modify the Reconcile function to compare the state specified by
// the BGPPeer object against the actual cluster state, and then
// perform operations to make the cluster state reflect the state specified by
// the user.
//
// For more details, check Reconcile and its Result here:
// - https://pkg.go.dev/sigs.k8s.io/controller-runtime@v0.14.1/pkg/reconcile
func (r *BGPPeerReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	peer := &sartv1alpha1.BGPPeer{}
	if err := r.Client.Get(ctx, req.NamespacedName, peer); err != nil {
		if apierrors.IsNotFound(err) {
			return r.reconcileWhenDelete(ctx, req)
		}
		logger.Error(err, "failed to get BGPPeer", "Object", req.NamespacedName)
		return ctrl.Result{}, err
	}

	// find bgp speaker
	nodeBGP := &sartv1alpha1.NodeBGP{}
	if peer.Spec.Node != "" {
		if err := r.Client.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: peer.Spec.Node}, nodeBGP); err != nil {
			if apierrors.IsNotFound(err) {
				return ctrl.Result{Requeue: true, RequeueAfter: time.Second * 5}, nil
			}
			logger.Error(err, "failed to get resource", "NodeBGP", types.NamespacedName{Namespace: constants.Namespace, Name: peer.Spec.Node})
			return ctrl.Result{}, err
		}
	} else {
		// if node is not specified, localAsn and localRouterId must be provided.
		nodeBGPList := &sartv1alpha1.NodeBGPList{}
		if err := r.Client.List(ctx, nodeBGPList); err != nil {
			logger.Error(err, "failed to list NodeBGP resource")
			return ctrl.Result{}, err
		}

		exist := false
		for _, nb := range nodeBGPList.Items {
			if nb.Spec.Asn == peer.Spec.LocalAsn && nb.Spec.RouterId == peer.Spec.LocalRouterId {
				nodeBGP = &nb
				exist = true
				break
			}
		}
		if !exist {
			err := fmt.Errorf("NodeBGP resource is not found")
			logger.Error(err, "failed to find wanted the NodeBGP resource", "ASN", peer.Spec.LocalAsn, "RouterId", peer.Spec.LocalRouterId)
			return ctrl.Result{}, err
		}
	}

	// confirm NodeBGP is available
	if nodeBGP.Status == sartv1alpha1.NodeBGPStatusUnavailable {
		logger.Info("NodeBGP is unavailable", "NodeBGP", nodeBGP.Spec)
		return ctrl.Result{Requeue: true, RequeueAfter: time.Second * 5}, nil
	}

	// create peer
	speakerEndpoint := speaker.New(r.SpeakerType, nodeBGP.Spec.RouterId, r.SpeakerEndpointPort)

	// check global config is set
	globalInfo, err := speakerEndpoint.GetInfo(ctx)
	if err != nil {
		logger.Error(err, "failed to get speaker information")
		return ctrl.Result{}, err
	}
	if globalInfo.Asn != nodeBGP.Spec.Asn || globalInfo.RouterId != nodeBGP.Spec.RouterId {
		err := fmt.Errorf("don't match speaker information")
		logger.Error(err, "", "info", globalInfo)
		return ctrl.Result{}, err
	}
	p, _ := speakerEndpoint.GetPeer(ctx, peer.Spec.PeerRouterId) // TODO: handle error if it is not not found error
	if p != nil {
		logger.Info("BGPPeer already exists", "LocalAsn", nodeBGP.Spec.Asn, "LocalRouterId", nodeBGP.Spec.RouterId, "PeerAsn", peer.Spec.PeerAsn, "PeerRouterId", peer.Spec.PeerRouterId)

		if peer.Status != sartv1alpha1.BGPPeerStatus(p.State) {
			peer.Status = sartv1alpha1.BGPPeerStatus(p.State)
			if err := r.Client.Status().Update(ctx, peer); err != nil {
				logger.Error(err, "failed to update status", "BGPPeer", peer)
				return ctrl.Result{}, err
			}
		}

		i, exist, needUpdate := isPeerRegistered(ctx, nodeBGP, peer)
		if exist {
			logger.Info("registered peer exists", "Node", nodeBGP, "Peer", peer)
			if needUpdate {
				logger.Info("need update", "Node", nodeBGP, "Peer", peer)
				nodeBGP.Spec.Peers[i] = PeerFromBGPPeer(peer)
			}
		} else {
			pp := PeerFromBGPPeer(peer)
			logger.Info("need register ", "Node", nodeBGP, "Peer", pp)
			nodeBGP.Spec.Peers = append(nodeBGP.Spec.Peers, pp)
		}
		if !exist || needUpdate {
			logger.Info("update node associated peers", "NodeBGP", nodeBGP, "Length of Peers", len(nodeBGP.Spec.Peers))
			if r.Client.Update(ctx, nodeBGP); err != nil {
				logger.Error(err, "failed to update NodeBGP to append peer", "Peer", peer, "NodeBGP", nodeBGP)
				return ctrl.Result{}, err
			}
		}

		switch peer.Status {
		case sartv1alpha1.BGPPeerStatusIdle, sartv1alpha1.BGPPeerStatusActive, sartv1alpha1.BGPPeerStatusConnect:
			return ctrl.Result{RequeueAfter: time.Minute * 5}, nil // TODO: find appropriate requeue time
		case sartv1alpha1.BGPPeerStatusOpenSent, sartv1alpha1.BGPPeerStatusOpenConfirm:
			return ctrl.Result{Requeue: true}, nil
		default:
			return ctrl.Result{}, nil
		}
	}

	// if a peer doesn't exist
	// update missing fields
	needUpdatePeer := false
	if peer.Spec.LocalAsn != nodeBGP.Spec.Asn || peer.Spec.LocalRouterId != nodeBGP.Spec.RouterId {
		peer.Spec.LocalAsn = nodeBGP.Spec.Asn
		peer.Spec.LocalRouterId = nodeBGP.Spec.RouterId
		needUpdatePeer = true
	}
	if peer.Spec.Node != nodeBGP.Name {
		peer.Spec.Node = nodeBGP.Name
		needUpdatePeer = true
	}

	// add peer to speaker
	if err := speakerEndpoint.AddPeer(ctx, speaker.PeerInfo{
		LocalAsn:      nodeBGP.Spec.Asn,
		LocalRouterId: nodeBGP.Spec.RouterId,
		PeerAsn:       peer.Spec.PeerAsn,
		PeerRouterId:  peer.Spec.PeerRouterId,
		Protocol:      peer.Spec.Family.Afi,
	}); err != nil {
		logger.Error(err, "failed to create peer by speaker", "Peer", peer, "NodeBGP", nodeBGP)
		return ctrl.Result{}, err
	}
	logger.Info("send BGP Peer create request", "Peer", peer, "NodeBGP", nodeBGP)

	i, exist, needUpdate := isPeerRegistered(ctx, nodeBGP, peer)
	if exist {
		logger.Info("registered peer exists", "Node", nodeBGP, "Peer", peer)
		if needUpdate {
			logger.Info("need register ", "Node", nodeBGP, "Peer", peer)
			nodeBGP.Spec.Peers[i] = PeerFromBGPPeer(peer)
		}
	} else {
		pp := PeerFromBGPPeer(peer)
		logger.Info("need register ", "Node", nodeBGP, "Peer", pp)
		nodeBGP.Spec.Peers = append(nodeBGP.Spec.Peers, pp)
	}
	if !exist || needUpdate {
		logger.Info("update node associated peers", "NodeBGP", nodeBGP, "Length of Peers", len(nodeBGP.Spec.Peers))
		if r.Client.Update(ctx, nodeBGP); err != nil {
			logger.Error(err, "failed to update NodeBGP to append peer", "Peer", peer, "NodeBGP", nodeBGP)
			return ctrl.Result{}, err
		}
	}

	if needUpdatePeer {
		if err := r.Client.Update(ctx, peer); err != nil {
			logger.Error(err, "failed to update BGPPeer")
			return ctrl.Result{}, err
		}
		logger.Info("complete BGPPeer information", "Peer", peer.Spec)
	}

	return ctrl.Result{Requeue: true}, nil
}

// SetupWithManager sets up the controller with the Manager.
func (r *BGPPeerReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&sartv1alpha1.BGPPeer{}).
		WithEventFilter(predicate.Funcs{
			UpdateFunc: func(ue event.UpdateEvent) bool {
				oldPeer := ue.ObjectOld.(*sartv1alpha1.BGPPeer).DeepCopy()
				newPeer := ue.ObjectNew.(*sartv1alpha1.BGPPeer).DeepCopy()
				// check advertisements
				oldPeer.Spec.Advertisements = nil
				newPeer.Spec.Advertisements = nil
				return !reflect.DeepEqual(oldPeer, newPeer)
			},
		}).
		Complete(r)
}

func (r *BGPPeerReconciler) reconcileWhenDelete(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	nodeBGPList := &sartv1alpha1.NodeBGPList{}
	if err := r.Client.List(ctx, nodeBGPList); err != nil {
		logger.Error(err, "failed to list NodeBGP resource")
		return ctrl.Result{}, err
	}
	for _, nb := range nodeBGPList.Items {
		idx := -1
		var peer sartv1alpha1.Peer
		for i, p := range nb.Spec.Peers {
			if req.Name == p.Name && req.Namespace == p.Namespace {
				idx = i
				peer = p
				break
			}
		}
		if idx != -1 {
			logger.Info("remove peer from NodeBGP", "Peer", req.NamespacedName, "NodeBGP", nb)
			nb.Spec.Peers = append(nb.Spec.Peers[:idx], nb.Spec.Peers[idx+1:]...)

			speakerEndpoint := speaker.New(r.SpeakerType, nb.Spec.RouterId, r.SpeakerEndpointPort)
			p, err := speakerEndpoint.GetPeer(ctx, peer.RouterId)
			if err != nil {
				logger.Error(err, "failed to get speaker peer", "NodeBGP", nb.Name, "Peer", peer.Name)
				return ctrl.Result{}, err
			}
			logger.Info("delete speaker peer", "Node", nb, "speaker peer", p)
			if err := speakerEndpoint.DeletePeer(ctx, peer.RouterId); err != nil {
				logger.Error(err, "failed to delete speaker peer", "NodeBGP", nb, "Peer", peer)
				return ctrl.Result{}, err
			}

			if err := r.Client.Update(ctx, &nb); err != nil {
				logger.Error(err, "failed to update NodeBGP", "NodeBGP", nb, "Peer", peer)
				return ctrl.Result{}, err
			}
			return ctrl.Result{}, nil
		}
	}

	err := fmt.Errorf("cann't find NodeBGP resource on all node")
	logger.Error(err, "search all nodes")
	return ctrl.Result{}, err
}

func PeerFromBGPPeer(p *sartv1alpha1.BGPPeer) sartv1alpha1.Peer {
	return sartv1alpha1.Peer{
		Name:      p.Name,
		Namespace: p.Namespace,
		Asn:       p.Spec.PeerAsn,
		RouterId:  p.Spec.PeerRouterId,
		Status:    p.Status,
	}
}

var (
	ErrPeerIsNotEstablished error = fmt.Errorf("Peer is not established")
)
