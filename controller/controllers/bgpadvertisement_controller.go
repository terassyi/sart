package controllers

import (
	"context"
	"fmt"
	"net"
	"strings"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/speaker"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/source"
)

// BGPAdvertisementReconciler reconciles a BGPAdvertisement object
type BGPAdvertisementReconciler struct {
	client.Client
	Scheme              *runtime.Scheme
	SpeakerEndpointPort uint32
	SpeakerType         speaker.SpeakerType
	advMap              map[string]map[string]bool
}

func NewBGPAdvertisementReconciler(client client.Client, scheme *runtime.Scheme, endpoint uint32, speakerType speaker.SpeakerType) *BGPAdvertisementReconciler {
	return &BGPAdvertisementReconciler{
		Client:              client,
		Scheme:              scheme,
		SpeakerEndpointPort: endpoint,
		SpeakerType:         speakerType,
		advMap:              map[string]map[string]bool{},
	}
}

// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements,verbs=get;list;watch;create;update;patch;delete
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements/finalizers,verbs=update
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=nodebgps,verbs=get;list
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgppeer/status,verbs=get;list

func (r *BGPAdvertisementReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {

	if err := r.reconcileBGPPeer(ctx, req); err != nil {
		if !apierrors.IsNotFound(err) {
			return ctrl.Result{}, err
		}
	}

	// recover advMap
	if r.advMap == nil || len(r.advMap) == 0 {
		if err := r.recover(ctx); err != nil {
			return ctrl.Result{}, err
		}
	}

	advertisement := &sartv1alpha1.BGPAdvertisement{}
	if err := r.Client.Get(ctx, req.NamespacedName, advertisement); err != nil {
		if apierrors.IsNotFound(err) {
			return r.reconcileWhenDelete(ctx, req)
		}
		return ctrl.Result{}, err
	}

	peerList := sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, &peerList); err != nil {
		return ctrl.Result{}, err
	}

	// check new or existing advertisement
	_, ok := r.advMap[types.NamespacedName{Namespace: advertisement.Namespace, Name: advertisement.Name}.String()]
	if !ok {
		// new advertisement
		if err := r.handleNewAdvertisement(ctx, advertisement); err != nil {
			return ctrl.Result{}, err
		}
		return ctrl.Result{}, nil
	}

	// existing advertisement
	if err := r.handleExistingAdvertisement(ctx, advertisement); err != nil {
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

func (r *BGPAdvertisementReconciler) reconcileBGPPeer(ctx context.Context, req ctrl.Request) error {

	peer := &sartv1alpha1.BGPPeer{}
	if err := r.Client.Get(ctx, req.NamespacedName, peer); err != nil {
		return err
	}

	peerAvailable := peer.Status == sartv1alpha1.BGPPeerStatusEstablished

	for name, infoMap := range r.advMap {
		infoMap[peer.Spec.Node] = peerAvailable

		available := func() sartv1alpha1.BGPAdvertisementCondition {
			if peerAvailable {
				return sartv1alpha1.BGPAdvertisementConditionAvailable
			}
			for _, a := range infoMap {
				if a {
					return sartv1alpha1.BGPAdvertisementConditionAvailable
				}
			}
			return sartv1alpha1.BGPAdvertisementConditionUnavailable
		}()

		namespacedName := strings.Split(name, "/")
		advertisement := &sartv1alpha1.BGPAdvertisement{}
		if err := r.Client.Get(ctx, types.NamespacedName{Namespace: namespacedName[0], Name: namespacedName[1]}, advertisement); err != nil {
			return err
		}

		if advertisement.Status.Condition != available {
			newAdv := advertisement.DeepCopy()
			newAdv.Status.Condition = available
			if err := r.Client.Status().Patch(ctx, newAdv, client.MergeFrom(advertisement)); err != nil {
				return err
			}
		}
	}
	return nil
}

func (r *BGPAdvertisementReconciler) recover(ctx context.Context) error {
	advList := &sartv1alpha1.BGPAdvertisementList{}
	if err := r.Client.List(ctx, advList); err != nil {
		return err
	}

	for _, adv := range advList.Items {
		nodeMap := make(map[string]bool)
		for _, n := range adv.Spec.Nodes {
			nodeMap[n] = true
		}
		r.advMap[types.NamespacedName{Namespace: adv.Namespace, Name: adv.Name}.String()] = nodeMap
	}
	return nil
}

func (r *BGPAdvertisementReconciler) handleNewAdvertisement(ctx context.Context, advertisement *sartv1alpha1.BGPAdvertisement) error {
	logger := log.FromContext(ctx)

	// create new advInfo
	advInfo := make(map[string]bool)

	peerList := &sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, peerList); err != nil {
		return err
	}

	for _, p := range peerList.Items {
		for _, target := range advertisement.Spec.Nodes {
			if p.Spec.Node == target {
				logger.Info("advertise new path", "Node", target, "Prefix", advertisement.Spec.Network)
				peerAvailable, err := r.advertise(ctx, &p, advertisement)
				if err != nil {
					return err
				}

				advInfo[p.Spec.Node] = peerAvailable
				contain := false
				for _, adv := range p.Spec.Advertisements {
					if adv.Name == advertisement.Name {
						contain = true
						break
					}
				}
				if !contain {
					p.Spec.Advertisements = append(p.Spec.Advertisements, sartv1alpha1.Advertisement{
						Name:      advertisement.Name,
						Namespace: advertisement.Namespace,
						Prefix:    advertisement.Spec.Network,
					})
					// update BGPPeer
					if err := r.Client.Update(ctx, &p); err != nil {
						return err
					}
				}
			}
		}
	}

	available := func() bool {
		for _, a := range advInfo {
			if a {
				return true
			}
		}
		return false
	}()

	newAdvertisement := advertisement.DeepCopy()
	if available {
		newAdvertisement.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
	} else {
		newAdvertisement.Status.Condition = sartv1alpha1.BGPAdvertisementConditionUnavailable
	}

	if err := r.Client.Status().Patch(ctx, newAdvertisement, client.MergeFrom(advertisement)); err != nil {
		return err
	}

	// register advertisement information
	r.advMap[types.NamespacedName{Namespace: advertisement.Namespace, Name: advertisement.Name}.String()] = advInfo

	return nil
}

func (r *BGPAdvertisementReconciler) handleExistingAdvertisement(ctx context.Context, advertisement *sartv1alpha1.BGPAdvertisement) error {
	logger := log.FromContext(ctx)

	var advInfo map[string]bool
	advInfo, ok := r.advMap[types.NamespacedName{Namespace: advertisement.Namespace, Name: advertisement.Name}.String()]
	if !ok {
		advInfo = make(map[string]bool)
		r.advMap[types.NamespacedName{Namespace: advertisement.Namespace, Name: advertisement.Name}.String()] = advInfo
	}

	peerList := &sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, peerList); err != nil {
		return err
	}
	// search advertised path
	added, removed := advDiff(*peerList, advertisement)

	for _, p := range removed {
		logger.Info("withdraw from", "Node", p.Spec.Node, "Advertisement", advertisement.Name, "Prefix", advertisement.Spec.Network)
		_, err := r.withdraw(ctx, &p, advertisement.Spec.Network)
		if err != nil {
			logger.Error(err, "failed to withdraw path by speaker", "Advertisement", advertisement)
			return err
		}

		delete(advInfo, p.Spec.Node)
		// remove from peer advertisement list
		index := -1
		for i, adv := range p.Spec.Advertisements {
			if adv.Name == advertisement.Name {
				index = i
				break
			}
		}
		if index != -1 {
			p.Spec.Advertisements = append(p.Spec.Advertisements[:index], p.Spec.Advertisements[index+1:]...)
			if err := r.Client.Update(ctx, &p); err != nil {
				return err
			}
		}

	}

	for _, p := range added {
		logger.Info("advertise to", "Node", p.Spec.Node, "Advertisement", advertisement.Name, "Prefix", advertisement.Spec.Network)
		peerAvailable, err := r.advertise(ctx, &p, advertisement)
		if err != nil {
			return err
		}

		advInfo[p.Spec.Node] = peerAvailable
		// append peer advertisement list
		contain := false
		for _, adv := range p.Spec.Advertisements {
			if adv.Name == advertisement.Name {
				contain = true
				break
			}
		}
		if !contain {
			p.Spec.Advertisements = append(p.Spec.Advertisements, sartv1alpha1.Advertisement{
				Name:      advertisement.Name,
				Namespace: advertisement.Namespace,
				Prefix:    advertisement.Spec.Network,
			})
			if err := r.Client.Update(ctx, &p); err != nil {
				return err
			}
		}
	}

	available := func() bool {
		for _, a := range advInfo {
			if a {
				return true
			}
		}
		return false
	}()

	newAdvertisement := advertisement.DeepCopy()
	if available {
		newAdvertisement.Status.Condition = sartv1alpha1.BGPAdvertisementConditionAvailable
	} else {
		newAdvertisement.Status.Condition = sartv1alpha1.BGPAdvertisementConditionUnavailable
	}

	if err := r.Client.Status().Patch(ctx, newAdvertisement, client.MergeFrom(advertisement)); err != nil {
		return err
	}

	// update advertisement information
	// r.advMap[types.NamespacedName{Namespace: advertisement.Namespace, Name: advertisement.Name}.String()] = advInfo

	return nil
}

func (r *BGPAdvertisementReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&sartv1alpha1.BGPAdvertisement{}).
		Watches(&source.Kind{Type: &sartv1alpha1.BGPPeer{}}, &handler.EnqueueRequestForObject{}).
		Complete(r)
}

func (r *BGPAdvertisementReconciler) reconcileWhenDelete(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	peerList := &sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, peerList); err != nil {
		return ctrl.Result{}, err
	}
	// find advertisements
	for _, p := range peerList.Items {
		remove := 0
		needUpdate := false
		for i, adv := range p.Spec.Advertisements {
			if req.Name == adv.Name && req.Namespace == adv.Namespace {
				// withdraw
				if _, err := r.withdraw(ctx, &p, adv.Prefix); err != nil {
					logger.Error(err, "failed to withdraw path by speaker", "Advertisement", adv)
					return ctrl.Result{}, err
				}
				remove = i
				needUpdate = true
				break
			}
		}
		if needUpdate {
			p.Spec.Advertisements = append(p.Spec.Advertisements[:remove], p.Spec.Advertisements[remove+1:]...)
			if err := r.Client.Update(ctx, &p); err != nil {
				return ctrl.Result{}, err
			}
		}
	}

	delete(r.advMap, req.NamespacedName.String())
	return ctrl.Result{}, nil
}

func (r *BGPAdvertisementReconciler) advertise(ctx context.Context, peer *sartv1alpha1.BGPPeer, adv *sartv1alpha1.BGPAdvertisement) (bool, error) {
	logger := log.FromContext(ctx)

	speakerEndpoint := speaker.New(r.SpeakerType, peer.Spec.LocalRouterId, r.SpeakerEndpointPort)
	peerInfo, err := speakerEndpoint.GetPeer(ctx, peer.Spec.PeerRouterId)
	if err != nil {
		logger.Error(err, "failed to get peer information", "Peer", peer.Spec)
		return false, err
	}
	if err := speakerEndpoint.AddPath(ctx, adv.ToPathInfo()); err != nil {
		logger.Error(err, "failed to add path to speaker", "Peer", peer.Spec, "Advertisement", adv.Spec)
		return false, err
	}

	if peerInfo.State == speaker.PeerStateEstablished {
		return true, nil
	}
	return false, nil
}

func (r *BGPAdvertisementReconciler) withdraw(ctx context.Context, peer *sartv1alpha1.BGPPeer, prefix string) (bool, error) {
	logger := log.FromContext(ctx)

	speakerEndpoint := speaker.New(r.SpeakerType, peer.Spec.LocalRouterId, r.SpeakerEndpointPort)
	peerInfo, err := speakerEndpoint.GetPeer(ctx, peer.Spec.PeerRouterId)
	if err != nil {
		logger.Error(err, "failed to get peer information", "Peer", peer.Spec)
		return false, err
	}
	addr, _, err := net.ParseCIDR(prefix)
	if err != nil {
		return false, err
	}
	pathInfo := speaker.PathInfo{
		Prefix: prefix,
	}
	if addr.To4() != nil {
		pathInfo.Protocol = "ipv4"
	} else if addr.To16() != nil {
		pathInfo.Protocol = "ipv6"
	} else {
		return false, fmt.Errorf("invalid protocol format")
	}

	if err := speakerEndpoint.DeletePath(ctx, pathInfo); err != nil {
		return false, err
	}

	if peerInfo.State == speaker.PeerStateEstablished {
		return true, nil
	}
	return false, nil
}

func advDiff(peerList sartv1alpha1.BGPPeerList, advertisement *sartv1alpha1.BGPAdvertisement) ([]sartv1alpha1.BGPPeer, []sartv1alpha1.BGPPeer) {
	peerMap := make(map[string]sartv1alpha1.BGPPeer)
	oldMap := make(map[string]sartv1alpha1.BGPPeer)
	for _, peer := range peerList.Items {
		peerMap[peer.Spec.Node] = peer
		for _, adv := range peer.Spec.Advertisements {
			if adv.Namespace == advertisement.Namespace && adv.Name == advertisement.Name {
				// advertised from this peer
				oldMap[peer.Spec.Node] = peer
			}
		}
	}

	removed := []sartv1alpha1.BGPPeer{}
	added := []sartv1alpha1.BGPPeer{}

	for _, an := range advertisement.Spec.Nodes {
		_, ok := oldMap[an]
		if !ok {
			added = append(added, peerMap[an])
		} else {
			delete(oldMap, an)
		}
	}

	for _, p := range oldMap {
		removed = append(removed, p)
	}

	return added, removed

}
