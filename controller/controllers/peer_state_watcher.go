package controllers

import (
	"context"
	"time"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/speaker"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/log"
)

type PeerStateWatcher struct {
	client.Client
	SpeakerEndpoint uint32
	SpeakerType     speaker.SpeakerType
	peerStateMap    map[string]*speaker.PeerInfo
	interval        uint32
}

func NewPeerStateWatcher(c client.Client, endpoint uint32, speakerType speaker.SpeakerType, interval uint32) *PeerStateWatcher {
	return &PeerStateWatcher{
		Client:          c,
		SpeakerEndpoint: endpoint,
		SpeakerType:     speakerType,
		peerStateMap:    make(map[string]*speaker.PeerInfo),
		interval:        interval,
	}
}

func (r *PeerStateWatcher) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	var peer *speaker.PeerInfo
	peer, ok := r.peerStateMap[req.NamespacedName.String()]
	if !ok {
		p := &sartv1alpha1.BGPPeer{}
		if err := r.Client.Get(ctx, req.NamespacedName, p); err != nil {
			return ctrl.Result{}, err
		}
		info := &speaker.PeerInfo{
			LocalAsn:      p.Spec.LocalAsn,
			LocalRouterId: p.Spec.LocalRouterId,
			PeerAsn:       p.Spec.PeerAsn,
			PeerRouterId:  p.Spec.PeerRouterId,
			Protocol:      p.Spec.Family.Afi,
			State:         speaker.PeerState(p.Status),
		}
		r.peerStateMap[req.NamespacedName.String()] = info
		peer = info
		logger.Info("register new peer to PeerStateWatcher")
	}

	speakerEndpoint := speaker.New(r.SpeakerType, peer.LocalRouterId, r.SpeakerEndpoint)
	newPeer, err := speakerEndpoint.GetPeer(ctx, peer.PeerRouterId)
	if err != nil {
		return ctrl.Result{}, err
	}

	logger.Info("watch peer state", "OldState", peer.State, "NewState", newPeer.State)
	if newPeer.State != peer.State {
		p := &sartv1alpha1.BGPPeer{}
		if err := r.Client.Get(ctx, req.NamespacedName, p); err != nil {
			return ctrl.Result{}, err
		}

		logger.Info("sync peer state", "OldState", peer.State, "NewState", newPeer.State)
		newP := p.DeepCopy()
		newP.Status = sartv1alpha1.BGPPeerStatus(newPeer.State)
		if err := r.Client.Status().Patch(ctx, newP, client.MergeFrom(p)); err != nil {
			return ctrl.Result{}, err
		}
	}

	return ctrl.Result{RequeueAfter: time.Second * time.Duration(r.interval*1000000000)}, nil
}

func (r *PeerStateWatcher) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&sartv1alpha1.BGPPeer{}).
		Complete(r)
}
