package controllers

import (
	"context"
	"errors"
	"fmt"
	"net/netip"
	"reflect"
	"sort"
	"strings"

	sartv1alpha1 "github.com/terassyi/sart/controller/api/v1alpha1"
	"github.com/terassyi/sart/controller/pkg/allocator"
	"github.com/terassyi/sart/controller/pkg/constants"
	v1 "k8s.io/api/core/v1"
	discoveryv1 "k8s.io/api/discovery/v1"
	apierrors "k8s.io/apimachinery/pkg/api/errors"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/types"
	ctrl "sigs.k8s.io/controller-runtime"
	"sigs.k8s.io/controller-runtime/pkg/client"
	"sigs.k8s.io/controller-runtime/pkg/controller/controllerutil"
	"sigs.k8s.io/controller-runtime/pkg/event"
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/predicate"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"
	"sigs.k8s.io/controller-runtime/pkg/source"
)

const (
	ServiceConditionTypeAdvertising string = "Advertising"
	ServiceConditionTypeAdvertised  string = "Advertised"
	ServiceConditionTypeNotReady    string = "NotReady"

	ServiceConditionReasonAdvertising string = "Advertising allocated LB addresses"
	ServiceConditionReasonAdvertised  string = "Advertised allocated LB addresses"
	ServiceConditionReasonNotReady    string = "No Ready Endpoints"

	AdvertisementTypeService string = "service"
)

var (
	ErrPoolAnnotationIsNotFound   error = errors.New("Pool name annotation is not found")
	ErrFromPoolAnnotationIsNotSet error = errors.New("Pool name allocated from is not set")
	ErrAllocatorIsNotFound        error = errors.New("Allocator is not found")
)

type LBAllocationReconciler struct {
	client.Client
	Allocators map[string]map[string]allocator.Allocator
	allocMap   map[string][]alloc
	recover    bool
}

func NewLBAllocationReconciler(client client.Client, allocators map[string]map[string]allocator.Allocator) *LBAllocationReconciler {
	return &LBAllocationReconciler{
		Client:     client,
		Allocators: allocators,
		allocMap:   make(map[string][]alloc),
		recover:    true,
	}
}

type alloc struct {
	pool     string
	name     string     // namespaced name of sartv1alpha1.BGPAdvertisement
	protocol string     // ipv4 or ipv6
	addr     netip.Addr // allocated address
}

func (a alloc) String() string {
	return fmt.Sprintf("%s/%s/%s/%s", a.pool, a.name, a.protocol, a.addr)
}

// +kubebuilder:rbac:groups="",resources=services,verbs=get;list;watch;update;patch
// +kubebuilder:rbac:groups="",resources=services/status,verbs=get;update;patch
// +kubebuilder:rbac:groups="",resources=endpoints,verbs=get;list;watch;update;patch
// +kubebuilder:rbac:groups="",resources=endpoints/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=discovery.k8s.io,resources=endpointslices,verbs=get;list;watch
// +kubebuilder:rbac:groups=discovery.k8s.io,resources=endpointslices/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools,verbs=get;list;watch;update;patch;delete
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools/status,verbs=get;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=addresspools/finalizers,verbs=update
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements,verbs=get;list;watch;create;update;patch;delete
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements/status,verbs=get;create;update;patch
// +kubebuilder:rbac:groups=sart.terassyi.net,resources=bgpadvertisements/finalizers,verbs=create;delete;update

func (r *LBAllocationReconciler) Reconcile(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	if r.recover {
		logger.Info("recover allocator information")
		if err := r.recoverAllocation(ctx); err != nil {
			return ctrl.Result{}, err
		}
	}

	if _, err := r.reconcileAddressPool(ctx, req); err != nil {
		logger.Error(err, "failed to reconcile AddressPool")
		return ctrl.Result{}, err
	}

	if _, err := r.reconcileService(ctx, req); err != nil {
		logger.Error(err, "failed to reconcile Service")
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) SetupWithManager(mgr ctrl.Manager) error {

	return ctrl.NewControllerManagedBy(mgr).
		For(&v1.Service{}).
		WithEventFilter(predicate.Funcs{
			CreateFunc: func(ce event.CreateEvent) bool {
				svc, ok := ce.Object.(*v1.Service)
				if !ok {
					return true
				}
				return filterService(svc) && !filterEmptyService(svc)
			},
			UpdateFunc: func(ue event.UpdateEvent) bool {
				svc, ok := ue.ObjectOld.(*v1.Service)
				if !ok {
					return true
				}
				return filterService(svc) && !filterEmptyService(svc)
			},
			DeleteFunc: func(de event.DeleteEvent) bool {
				svc, ok := de.Object.(*v1.Service)
				if !ok {
					return true
				}
				return filterService(svc) && !filterEmptyService(svc)
			},
		}).
		Watches(&source.Kind{Type: &sartv1alpha1.AddressPool{}}, &handler.EnqueueRequestForObject{}).
		Watches(&source.Kind{Type: &discoveryv1.EndpointSlice{}}, handler.EnqueueRequestsFromMapFunc(func(o client.Object) []reconcile.Request {
			epSlice, ok := o.(*discoveryv1.EndpointSlice)
			if !ok {
				return []reconcile.Request{}
			}
			serviceName, err := serviceNameFromEndpointSlice(epSlice)
			if err != nil {
				return []reconcile.Request{}
			}
			return []reconcile.Request{{NamespacedName: serviceName}}
		})).
		Watches(&source.Kind{Type: &sartv1alpha1.BGPAdvertisement{}}, handler.EnqueueRequestsFromMapFunc(func(o client.Object) []reconcile.Request {
			adv, ok := o.(*sartv1alpha1.BGPAdvertisement)
			if !ok {
				return []reconcile.Request{}
			}
			if adv.Spec.Type != "service" {
				return []reconcile.Request{}
			}
			return []reconcile.Request{{NamespacedName: types.NamespacedName{Namespace: adv.Namespace, Name: advertiseNameToServiceName(adv.Name)}}}
		})).
		Complete(r)
}

func (r *LBAllocationReconciler) reconcileAddressPool(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	addressPool := &sartv1alpha1.AddressPool{}
	if err := r.Get(ctx, req.NamespacedName, addressPool); err != nil {
		if apierrors.IsNotFound(err) {
			return ctrl.Result{}, nil
		}
		logger.Error(err, "failed to get addresspool", "name", addressPool.Name)
		return ctrl.Result{}, err
	}

	// add finalizer
	// finalizer := constants.GetFinalizerName(addressPool.TypeMeta)
	if !controllerutil.ContainsFinalizer(addressPool, constants.AddressPoolFinalizer) {
		controllerutil.AddFinalizer(addressPool, constants.AddressPoolFinalizer)
		if err := r.Client.Update(ctx, addressPool); err != nil {
			return ctrl.Result{}, err
		}
	}
	// remove finalizer and delete addresspool
	if !addressPool.ObjectMeta.DeletionTimestamp.IsZero() {
		logger.Info("deletion timestamp is not zero. remove AddressPool", "Name", addressPool.Name, "DeletionTimestamp", addressPool.DeletionTimestamp)
		if controllerutil.ContainsFinalizer(addressPool, constants.AddressPoolFinalizer) {
			controllerutil.RemoveFinalizer(addressPool, constants.AddressPoolFinalizer)

			delete(r.Allocators, addressPool.Name)

			if err := r.Client.Update(ctx, addressPool); err != nil {
				logger.Error(err, "failed to remove finalizer")
				return ctrl.Result{}, err
			}
		}
		return ctrl.Result{}, nil
	}

	if addressPool.Spec.Type != "lb" {
		return ctrl.Result{}, nil
	}

	p, ok := r.Allocators[addressPool.Name]
	if !ok {
		// create AddressaddressPool
		protocolAllocator := make(map[string]allocator.Allocator)
		for _, cidr := range addressPool.Spec.Cidrs {
			prefix, err := netip.ParsePrefix(cidr.Prefix)
			if err != nil {
				logger.Error(err, "failed to parse CIDR", "AddressaddressPool.Spec.Cidr", cidr)
				return ctrl.Result{}, err
			}
			protocolAllocator[cidr.Protocol] = allocator.New(&prefix)
		}
		logger.Info("create an allocator", "CIDRs", addressPool.Spec.Cidrs, "Disabled", addressPool.Spec.Disable)
		r.Allocators[addressPool.Name] = protocolAllocator
		return ctrl.Result{}, nil
	}

	for _, a := range p {
		if addressPool.Spec.Disable {
			a.Disable()
		} else {
			a.Enable()
		}
	}
	return ctrl.Result{}, nil
}

// reconcileService is handle v1.Service and discoveryv1.EndpointSlice
func (r *LBAllocationReconciler) reconcileService(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	svc := &v1.Service{}
	if err := r.Client.Get(ctx, req.NamespacedName, svc); err != nil {
		// Reconciliation loop targets is v1.Service, discoveryv1.EndpointSlice and sartv1alpha1.BGPAdvertisement.
		// but ignore the deletion of EndpointSlice and BGPAdvertisement
		if apierrors.IsNotFound(err) {
			// handle deletion
			return r.reconcileServiceWhenDelete(ctx, req)
		}
		logger.Error(err, "failed to get Service", "Name", req.NamespacedName)
		return ctrl.Result{}, err
	}

	// filter Load balancer type
	if svc.Spec.Type != v1.ServiceTypeLoadBalancer {
		return ctrl.Result{}, nil
	}

	if err := r.handleService(ctx, svc); err != nil {
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

// reconsileServiceWhenDelete: the target of reconciling is v1.Service
func (r *LBAllocationReconciler) reconcileServiceWhenDelete(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	allocInfo, ok := r.allocMap[req.NamespacedName.String()]
	if !ok {
		// if allocations are not found, there is nothing to do
		return ctrl.Result{}, nil
	}

	// release allocations
	// Service resource must be deleted
	if len(allocInfo) == 0 {
		return ctrl.Result{}, fmt.Errorf("Allocation must be found at least one")
	}

	pool := allocInfo[0].pool
	allocators, ok := r.Allocators[pool]
	if !ok {
		return ctrl.Result{}, ErrAllocatorIsNotFound
	}

	for _, a := range allocInfo {
		allocator, ok := allocators[a.protocol]
		if !ok {
			return ctrl.Result{}, ErrAllocatorIsNotFound
		}
		res, err := allocator.Release(a.addr)
		if err != nil {
			return ctrl.Result{}, err
		}
		if !res {
			continue
		}
		logger.Info("Release LB address", "Address", a.addr)

		// delete BGPAdvertisement associated with
		adv := &sartv1alpha1.BGPAdvertisement{}
		if err := r.Client.Get(ctx, types.NamespacedName{Namespace: req.Namespace, Name: a.name}, adv); err != nil {
			if apierrors.IsNotFound(err) {
				logger.Info("BGPAdvertisement is not found", "Name", a.name)
				continue
			}
			return ctrl.Result{}, err
		}
		if err := r.Client.Delete(ctx, adv); err != nil {
			return ctrl.Result{}, err
		}
	}

	// delete allocation information from allocMap
	delete(r.allocMap, req.NamespacedName.String())

	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) handleService(ctx context.Context, svc *v1.Service) error {
	logger := log.FromContext(ctx)

	pool, addrs, err := r.isAssigned(svc)
	if err != nil {
		return err
	}
	if addrs != nil {
		// already assigned
		logger.Info("LB address is already assigned", "Policy", svc.Spec.ExternalTrafficPolicy, "Pool", pool, "Address", addrs)
		// handle advertising endpoints changes
		return r.handleEndpointUpdate(ctx, svc)
	}

	_, res, err := r.isAllocated(svc)
	if err != nil {
		return err
	}
	if res {
		// allocated but not assigned yet
		logger.Info("LB address is allocated but not assigned", "Policy", svc.Spec.ExternalTrafficPolicy, "Pool", pool)
		// assign
		return r.assign(ctx, svc)
	}

	endpoints, err := r.getHealthyEndpoints(ctx, svc)
	if err != nil {
		return err
	}
	// check any endpoints are healthy
	// if there are no healthy endpoints
	if len(endpoints) == 0 {
		logger.Info("There are no ready endpoints")
		newSvc := svc.DeepCopy()
		newSvc.Status.Conditions = append(newSvc.Status.Conditions, metav1.Condition{
			Type:    ServiceConditionTypeNotReady,
			Status:  metav1.ConditionFalse,
			Reason:  ServiceConditionReasonNotReady,
			Message: "There are no ready endpoints",
		})
		if err := r.Client.Status().Patch(ctx, newSvc, client.MergeFrom(svc)); err != nil {
			logger.Error(err, "failed to update service status")
			return err
		}
		return nil
	}

	// not allocated
	allocated, err := r.allocate(svc, pool)
	if err != nil {
		return err
	}

	// create advertisement info
	for _, a := range allocated {
		adv, needUpdate, err := r.createOrUpdateAdvertisement(ctx, a.name, svc, a.addr, endpoints)
		if err != nil {
			return err
		}
		if adv != nil {
			if needUpdate {
				// update BGPAdvertisement
				if err := r.Client.Update(ctx, adv); err != nil {
					return err
				}
				// update BGPAdvertisement.status
				if err := r.Client.Status().Update(ctx, adv); err != nil {
					return err
				}

			} else {
				// create new BGPAdvertisement
				if err := r.Client.Create(ctx, adv); err != nil {
					return err
				}
			}
		}
	}

	newSvc := svc.DeepCopy()

	newSvc.Status.Conditions = append(newSvc.Status.Conditions, metav1.Condition{
		Type:    ServiceConditionTypeAdvertising,
		Status:  metav1.ConditionFalse,
		Reason:  ServiceConditionReasonAdvertising,
		Message: "LB addresses are allocated internally",
	})
	newSvc.Annotations[constants.AnnotationAllocatedFromPool] = pool

	logger.Info("Allocate LB adddress", "Pool", pool)
	if err := r.Client.Patch(ctx, newSvc, client.MergeFrom(svc)); err != nil {
		logger.Error(err, "failed to update service")
		return err
	}
	if err := r.Client.Status().Patch(ctx, newSvc, client.MergeFrom(svc)); err != nil {
		logger.Error(err, "failed to update service status")
		return err
	}
	return nil
}

func (r *LBAllocationReconciler) handleEndpointUpdate(ctx context.Context, svc *v1.Service) error {
	logger := log.FromContext(ctx)

	nodes, err := r.getHealthyEndpoints(ctx, svc)
	if err != nil {
		return err
	}

	advertisements := []*sartv1alpha1.BGPAdvertisement{}
	for _, ingress := range svc.Status.LoadBalancer.Ingress {
		addr, err := netip.ParseAddr(ingress.IP)
		if err != nil {
			return err
		}
		advName := svcToAdvertisementName(svc, protocolFromAddr(addr))
		adv := &sartv1alpha1.BGPAdvertisement{}
		if err := r.Client.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: advName}, adv); err != nil {
			return err
		}
		advertisements = append(advertisements, adv)
	}

	for _, adv := range advertisements {
		if reflect.DeepEqual(adv.Spec.Nodes, nodes) {
			return nil
		}
		logger.Info("need to update endpoints", "old", adv.Spec.Nodes, "new", nodes)
		newAdv := adv.DeepCopy()
		newAdv.Spec.Nodes = nodes
		if err := r.Client.Patch(ctx, newAdv, client.MergeFrom(adv)); err != nil {
			return err
		}
		newAdv.Status = sartv1alpha1.BGPAdvertisementStatus{
			Condition:   sartv1alpha1.BGPAdvertisementConditionUpdated,
			Advertising: uint32(len(nodes)),
			Advertised:  0,
		}
		if err := r.Client.Status().Patch(ctx, newAdv, client.MergeFrom(adv)); err != nil {
			return err
		}
	}
	return nil
}

func (r *LBAllocationReconciler) allocate(svc *v1.Service, pool string) ([]alloc, error) {
	allocators, ok := r.Allocators[pool]
	if !ok {
		return nil, ErrAllocatorIsNotFound
	}
	desired, err := desiredAddr(svc)
	if err != nil {
		return nil, err
	}

	for proto, allocator := range allocators {
		addr, ok := desired[proto]
		if ok {
			// desired address is specified
			_, err := allocator.Allocate(addr)
			if err != nil {
				return nil, err
			}
		} else {
			addr, err := allocator.AllocateNext()
			if err != nil {
				return nil, err
			}
			desired[proto] = addr
		}
	}
	// set allocation info
	allocInfo := make([]alloc, 0, len(desired))
	for proto, addr := range desired {
		a := alloc{
			pool:     pool,
			name:     svcToAdvertisementName(svc, proto),
			protocol: proto,
			addr:     addr,
		}
		allocInfo = append(allocInfo, a)
	}
	r.allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()] = allocInfo
	return allocInfo, nil
}

func (r *LBAllocationReconciler) assign(ctx context.Context, svc *v1.Service) error {
	logger := log.FromContext(ctx)
	svcName := types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}

	allocs, ok := r.allocMap[svcName.String()]
	if !ok {
		return fmt.Errorf("Allocation is not found")
	}

	newSvc := svc.DeepCopy()
	if len(newSvc.Status.LoadBalancer.Ingress) != 0 {
		newSvc.Status.LoadBalancer = v1.LoadBalancerStatus{}
	}

	assigned := false
	for _, a := range allocs {
		adv := &sartv1alpha1.BGPAdvertisement{}
		if err := r.Client.Get(ctx, types.NamespacedName{Namespace: svc.Namespace, Name: a.name}, adv); err != nil {
			if apierrors.IsNotFound(err) {
				logger.Info("hoge", "Resource", types.NamespacedName{Namespace: svc.Namespace, Name: a.name}.String())
				return err
			}
			logger.Error(err, "failed to get BGPAdvertisement")
			return err
		}
		if adv.Status.Advertised == 0 {
			logger.Info("advertisements is not ready")
			return nil
		}

		prefix, err := netip.ParsePrefix(adv.Spec.Network)
		if err != nil {
			return err
		}
		if a.addr != prefix.Addr() {
			return fmt.Errorf("Advertisement and allocation information is not matched")
		}
		newSvc.Status.LoadBalancer.Ingress = append(newSvc.Status.LoadBalancer.Ingress, v1.LoadBalancerIngress{
			IP: prefix.Addr().String(),
		})
		assigned = true
	}
	logger.Info("decide to assign", "assign", assigned)
	if assigned {
		newSvc.Status.Conditions = append(newSvc.Status.Conditions, metav1.Condition{
			Type:    ServiceConditionTypeAdvertised,
			Status:  metav1.ConditionTrue,
			Reason:  ServiceConditionReasonAdvertised,
			Message: "allocated addresses are advertised and assigned",
		})
	}

	if reflect.DeepEqual(svc, newSvc) {
		return nil
	}

	addrs := make([]string, 0, 2)
	for _, a := range allocs {
		addrs = append(addrs, a.addr.String())
	}
	logger.Info("Assign LB addresses", "Address", addrs)
	if err := r.Client.Status().Patch(ctx, newSvc, client.MergeFrom(svc)); err != nil {
		return err
	}

	return nil
}

func (r *LBAllocationReconciler) isAssigned(svc *v1.Service) (string, map[string]netip.Addr, error) {
	// check protocol by clusterIP
	if len(svc.Spec.ClusterIP) == 0 && svc.Spec.ClusterIP == "" {
		return "", nil, fmt.Errorf("Service doesn't have ClusterIP")
	}

	poolName, ok := svc.Annotations[constants.AnnotationAddressPool]
	if !ok {
		return "", nil, fmt.Errorf("required annotation %s is not found to address allocated load balancer", constants.AnnotationAddressPool)
	}

	lbIPs := make(map[string]netip.Addr)
	// check allocated LB IP
	for _, ingress := range svc.Status.LoadBalancer.Ingress {
		addr, err := netip.ParseAddr(ingress.IP)
		if err != nil {
			return "", nil, err
		}
		lbIPs[protocolFromAddr(addr)] = addr
	}

	if len(lbIPs) == 0 {
		// not allocated yet
		return poolName, nil, nil
	}

	from, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
	if !ok {
		return "", nil, fmt.Errorf("required annotation %s is not found to address allocated load balancer", constants.AnnotationAllocatedFromPool)
	}
	if poolName != from {
		return "", nil, fmt.Errorf("required pool is %s, actual allocating pool is %s", poolName, from)
	}

	protocolAllocator, ok := r.Allocators[poolName]
	if !ok {
		return "", nil, fmt.Errorf("Allocator is not registered: %s", poolName)
	}
	for proto, addr := range lbIPs {
		if !protocolAllocator[proto].IsAllocated(addr) {
			return poolName, nil, fmt.Errorf("address is not allocated %s", addr)
		}
	}

	return poolName, lbIPs, nil
}

func (r *LBAllocationReconciler) isAllocated(svc *v1.Service) ([]alloc, bool, error) {
	poolName, ok := svc.Annotations[constants.AnnotationAddressPool]
	if !ok {
		return nil, false, ErrPoolAnnotationIsNotFound
	}
	from, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
	if !ok {
		// not allocated yet
		return nil, false, nil
	}
	if poolName != from {
		return nil, false, fmt.Errorf("pool name is not matched")
	}

	allocs, ok := r.allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()]
	if !ok {
		return nil, false, nil
	}

	for _, alloc := range allocs {
		allocators, ok := r.Allocators[poolName]
		if !ok {
			return nil, false, ErrAllocatorIsNotFound
		}
		allocator, ok := allocators[alloc.protocol]
		if !ok {
			return nil, false, ErrAllocatorIsNotFound
		}
		if !allocator.IsAllocated(alloc.addr) {
			return nil, false, fmt.Errorf("expected allocation is not satisfied %s", alloc)
		}
	}

	return allocs, true, nil
}

func desiredAddr(svc *v1.Service) (map[string]netip.Addr, error) {
	desired := make(map[string]netip.Addr)
	if svc.Spec.LoadBalancerIP == "" {
		// no desired addresses
		return desired, nil
	}

	ipStrs := strings.Split(svc.Spec.LoadBalancerIP, ",")
	for _, is := range ipStrs {
		ip, err := netip.ParseAddr(is)
		if err != nil {
			return nil, err
		}
		if ip.Is4() {
			desired[constants.ProtocolIpv4] = ip
		} else {
			desired[constants.ProtocolIpv6] = ip
		}
	}
	return desired, nil
}

func (r *LBAllocationReconciler) createOrUpdateAdvertisement(ctx context.Context, advName string, svc *v1.Service, lbAddr netip.Addr, endpointNodes []string) (*sartv1alpha1.BGPAdvertisement, bool, error) {
	logger := log.FromContext(ctx)

	prefix := netip.PrefixFrom(lbAddr, 32)

	needUpdate := false
	advertisement := &sartv1alpha1.BGPAdvertisement{}
	if err := r.Client.Get(ctx, types.NamespacedName{Name: advName, Namespace: svc.Namespace}, advertisement); err != nil {
		if apierrors.IsNotFound(err) {
			// advertise is not exist
			// create it
			needUpdate = true

			advertisement.Name = advName
			advertisement.Namespace = svc.Namespace

			spec := sartv1alpha1.BGPAdvertisementSpec{
				Network:   prefix.String(),
				Type:      AdvertisementTypeService,
				Protocol:  protocolFromAddr(lbAddr),
				Origin:    "",
				LocalPref: 0,
				Nodes:     endpointNodes,
			}
			advertisement.Spec = spec
			advertisement.Status = sartv1alpha1.BGPAdvertisementStatus{
				Condition:   sartv1alpha1.BGPAdvertisementConditionAdvertising,
				Advertising: uint32(len(endpointNodes)),
				Advertised:  0,
			}
			// set owner reference
			if err := controllerutil.SetOwnerReference(svc, advertisement, r.Scheme()); err != nil {
				return nil, false, err
			}
			return advertisement, false, nil
		} else {
			logger.Error(err, "failed to get BgpAdvertisement", "Name", advName)
			return nil, false, err
		}
	}

	if prefix.String() != advertisement.Spec.Network {
		advertisement.Spec.Network = prefix.String()
		needUpdate = true
	}
	if !reflect.DeepEqual(advertisement.Spec.Nodes, endpointNodes) {
		advertisement.Spec.Nodes = endpointNodes
		advertisement.Status.Advertised = 0
		advertisement.Status.Advertising = uint32(len(endpointNodes))
		needUpdate = true
	}

	if !needUpdate {
		return nil, false, nil
	}

	return advertisement, true, nil
}

func (r *LBAllocationReconciler) getHealthyEndpoints(ctx context.Context, svc *v1.Service) ([]string, error) {
	switch svc.Spec.ExternalTrafficPolicy {
	case v1.ServiceExternalTrafficPolicyTypeCluster:
		return r.getHealthyClusterEndpoints(ctx, svc)
	case v1.ServiceExternalTrafficPolicyTypeLocal:
		return r.getHealthyLocalEndpoints(ctx, svc)
	}
	return nil, nil
}

func (r *LBAllocationReconciler) getHealthyLocalEndpoints(ctx context.Context, svc *v1.Service) ([]string, error) {

	epsList := &discoveryv1.EndpointSliceList{}
	if svc.Spec.ExternalTrafficPolicy == v1.ServiceExternalTrafficPolicyTypeLocal {
		if err := r.Client.List(ctx, epsList, client.MatchingLabels{constants.KubernetesServiceNameLabel: svc.Name}); err != nil {
			return nil, err
		}
	}

	endpointNodeMap := make(map[string]struct{})

	for _, eps := range epsList.Items {
		for _, ep := range eps.Endpoints {
			if ep.Conditions.Ready != nil && !*ep.Conditions.Ready {
				// filter NotReady endpoint
				continue
			}
			if ep.NodeName == nil {
				continue
			}
			if *ep.NodeName == "" {
				continue
			}
			endpointNodeMap[*ep.NodeName] = struct{}{}
		}
	}
	endpointNodes := make([]string, 0, len(endpointNodeMap))
	for n, _ := range endpointNodeMap {
		endpointNodes = append(endpointNodes, n)
	}
	sort.Strings(endpointNodes)
	return endpointNodes, nil
}

func (r *LBAllocationReconciler) getHealthyClusterEndpoints(ctx context.Context, svc *v1.Service) ([]string, error) {

	ready := make(map[string]bool)

	epsList := &discoveryv1.EndpointSliceList{}
	if svc.Spec.ExternalTrafficPolicy == v1.ServiceExternalTrafficPolicyTypeCluster {
		if err := r.Client.List(ctx, epsList, client.MatchingLabels{constants.KubernetesServiceNameLabel: svc.Name}); err != nil {
			return nil, err
		}
	}
	for _, eps := range epsList.Items {
		for _, ep := range eps.Endpoints {
			if ep.Conditions.Ready != nil && !*ep.Conditions.Ready {
				// filter NotReady endpoint
				continue
			}
			for _, addr := range ep.Addresses {
				ready[addr] = true
			}
		}
	}
	if len(ready) == 0 {
		return []string{}, nil
	}

	peerList := &sartv1alpha1.BGPPeerList{}
	if err := r.Client.List(ctx, peerList); err != nil {
		return nil, err
	}

	peerNodes := make(map[string]struct{})
	endpointNodes := []string{}
	for _, p := range peerList.Items {
		peerNodes[p.Spec.Node] = struct{}{}
	}
	for k, _ := range peerNodes {
		endpointNodes = append(endpointNodes, k)
	}
	sort.Strings(endpointNodes)
	return endpointNodes, nil
}

func (r *LBAllocationReconciler) recoverAllocation(ctx context.Context) error {
	logger := log.FromContext(ctx)

	logger.Info("recover allocators")

	poolList := &sartv1alpha1.AddressPoolList{}
	if err := r.Client.List(ctx, poolList); err != nil {
		return err
	}
	for _, p := range poolList.Items {
		if p.Spec.Type != "lb" {
			continue
		}
		if _, ok := r.Allocators[p.Name]; ok {
			continue
		}
		allocators := make(map[string]allocator.Allocator)
		for _, cidr := range p.Spec.Cidrs {
			prefix, err := netip.ParsePrefix(cidr.Prefix)
			if err != nil {
				return err
			}
			allocators[cidr.Protocol] = allocator.New(&prefix)
		}
		r.Allocators[p.Name] = allocators
	}

	svcList := &v1.ServiceList{}
	if err := r.Client.List(ctx, svcList); err != nil {
		return err
	}
	advList := &sartv1alpha1.BGPAdvertisementList{}
	if err := r.Client.List(ctx, advList); err != nil {
		return err
	}
	advMap := make(map[string]sartv1alpha1.BGPAdvertisement)
	for _, a := range advList.Items {
		advMap[a.Name] = a
	}

	for _, svc := range svcList.Items {
		if svc.Spec.Type != v1.ServiceTypeLoadBalancer {
			continue
		}
		poolName, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
		if !ok {
			continue
		}
		allocators, ok := r.Allocators[poolName]
		if !ok {
			continue
		}
		alloInfo := make([]alloc, 0, 2)
		for proto, a := range allocators {
			adv, ok := advMap[svcToAdvertisementName(&svc, proto)]
			if !ok {
				continue
			}
			prefix, err := netip.ParsePrefix(adv.Spec.Network)
			if err != nil {
				return err
			}
			if !a.IsAllocated(prefix.Addr()) {
				if _, err := a.Allocate(prefix.Addr()); err != nil {
					return err
				}
			}
			alloInfo = append(alloInfo, alloc{
				pool:     poolName,
				name:     adv.Name,
				protocol: proto,
				addr:     prefix.Addr(),
			})
		}
		r.allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()] = alloInfo
	}
	r.recover = false
	return nil
}

func serviceNameFromEndpointSlice(epSlice *discoveryv1.EndpointSlice) (types.NamespacedName, error) {
	if epSlice == nil {
		return types.NamespacedName{}, fmt.Errorf("EndpointSlice is nil")
	}
	svcName, ok := epSlice.Labels[discoveryv1.LabelServiceName]
	if !ok || svcName == "" {
		return types.NamespacedName{}, fmt.Errorf("ServiceName is not found in labels")
	}
	return types.NamespacedName{Namespace: epSlice.Namespace, Name: svcName}, nil
}

func protocolFromAddr(addr netip.Addr) string {
	if addr.Is4() {
		return constants.ProtocolIpv4
	}
	return constants.ProtocolIpv6
}

func svcToAdvertisementName(svc *v1.Service, protocol string) string {
	return svc.Name + "-" + protocol
}

func advertiseNameToServiceName(adv string) string {
	name := strings.TrimSuffix(adv, "-"+constants.ProtocolIpv4)
	name = strings.TrimSuffix(name, "-"+constants.ProtocolIpv6)

	return name
}

func filterService(svc *v1.Service) bool {
	return svc.Spec.Type == v1.ServiceTypeLoadBalancer
}

func filterEmptyService(svc *v1.Service) bool {
	return svc.Name == "" && svc.Namespace == ""
}
