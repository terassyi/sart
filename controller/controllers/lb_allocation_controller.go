package controllers

import (
	"context"
	"errors"
	"fmt"
	"net/netip"
	"reflect"
	"sort"
	"strings"

	"github.com/go-logr/logr"
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
	"sigs.k8s.io/controller-runtime/pkg/handler"
	"sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/reconcile"
	"sigs.k8s.io/controller-runtime/pkg/source"
)

const (
	ServiceConditionTypeAdvertising string = "Advertising"
	ServiceConditionTypeAdvertised  string = "Advertised"

	ServiceConditionReasonAdvertising string = "Advertising allocated LB addresses"
	ServiceConditionReasonAdvertised  string = "Advertised allocated LB addresses"

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
}

type alloc struct {
	name     string     // namespaced name of sartv1alpha1.BGPAdvertisement
	protocol string     // ipv4 or ipv6
	addr     netip.Addr // allocated address
}

func (a alloc) String() string {
	return fmt.Sprintf("%s/%s/%s", a.name, a.protocol, a.addr)
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

	if _, err := r.reconcileAddressPool(ctx, req); err != nil {
		logger.Error(err, "failed to reconcile AddressPool")
		return ctrl.Result{}, err
	}

	if _, err := r.reconcileService(ctx, req); err != nil {
		logger.Error(err, "failed to reconcile Service")
		return ctrl.Result{}, err
	}
	// handle BGPAdvertisement.Status
	if err := r.assign(ctx, req); err != nil {
		if apierrors.IsNotFound(err) {
			return ctrl.Result{}, nil
		}
		logger.Error(err, "failed to watch BGPAdvertisement", "Name", req)
		return ctrl.Result{}, err
	}

	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) SetupWithManager(mgr ctrl.Manager) error {
	return ctrl.NewControllerManagedBy(mgr).
		For(&v1.Service{}).
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
			ns, name := advertiseNameToServiceName(adv.Name)
			return []reconcile.Request{{NamespacedName: types.NamespacedName{Namespace: ns, Name: name}}}
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
	finalizer := constants.GetFinalizerName(addressPool.TypeMeta)
	if !controllerutil.ContainsFinalizer(addressPool, finalizer) {
		controllerutil.AddFinalizer(addressPool, finalizer)
		if err := r.Client.Update(ctx, addressPool); err != nil {
			return ctrl.Result{}, err
		}
	}
	// remove finalizer and delete addresspool
	if !addressPool.ObjectMeta.DeletionTimestamp.IsZero() {
		logger.Info("deletion timestamp is not zero. remove AddressPool", "Name", addressPool.Name, "DeletionTimestamp", addressPool.DeletionTimestamp)
		if controllerutil.ContainsFinalizer(addressPool, finalizer) {
			controllerutil.RemoveFinalizer(addressPool, finalizer)

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

	logger.Info("AddressaddressPool is already created", "AddressaddressPool", addressPool.Name, "allocator", p)
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

	switch svc.Spec.ExternalTrafficPolicy {
	case v1.ServiceExternalTrafficPolicyTypeCluster:
		if err := r.handleLocal(ctx, svc); err != nil {
			return ctrl.Result{}, err
		}
	case v1.ServiceExternalTrafficPolicyTypeLocal:
		if err := r.handleLocal(ctx, svc); err != nil {
			return ctrl.Result{}, err
		}
	}

	// list endpoint slices
	epSliceList := discoveryv1.EndpointSliceList{}
	if err := r.Client.List(ctx, &epSliceList, client.MatchingLabels{constants.KubernetesServiceNameLabel: req.Name}); err != nil {
		logger.Error(err, "failed to list EndpointSlice by Service", "ServiceName", req.NamespacedName.String())
		return ctrl.Result{}, err
	}

	// allocation
	newSvc := svc.DeepCopy()

	poolName, allocatedAddrs, err := r.allocate(ctx, newSvc, epSliceList)
	if err != nil {
		logger.Error(err, "failed to allocate a LB address", "Name", req.NamespacedName)
		// clear allocation information from svc
		if err := r.release(logger, newSvc); err != nil {
			logger.Error(err, "failed to release", "Name", req.NamespacedName)
			return ctrl.Result{}, err
		}
		return ctrl.Result{}, err
	}
	if err := r.setAllocationInfo(newSvc, poolName, allocatedAddrs); err != nil {
		logger.Error(err, "failed to set allocation information", "Name", req.NamespacedName)
		return ctrl.Result{}, err
	}

	// create advertisement
	for _, ip := range allocatedAddrs {
		advName := svcToAdvertisementName(svc, protocolFromAddr(ip))
		adv, needUpdate, err := r.createOrUpdateAdvertisement(ctx, advName, ip, epSliceList)
		if err != nil {
			return ctrl.Result{}, err
		}
		if adv != nil {
			if needUpdate {
				if err := r.Client.Update(ctx, adv); err != nil {
					logger.Error(err, "failed to update advertisement", "Name", adv.Name)
					return ctrl.Result{}, err
				}
			} else {
				if err := r.Client.Create(ctx, adv); err != nil {
					logger.Error(err, "failed to create advertisement", "Name", adv.Name)
					return ctrl.Result{}, err
				}
				conditionAdvertising := metav1.Condition{
					Type:    ServiceConditionTypeAdvertising,
					Status:  metav1.ConditionFalse,
					Reason:  ServiceConditionReasonAdvertising,
					Message: "LB addresses are advertising by sart",
				}
				newSvc.Status.Conditions = append(newSvc.Status.Conditions, conditionAdvertising)
			}
		}
	}

	if !reflect.DeepEqual(svc.Annotations, newSvc.Annotations) {
		// update all
		if err := r.Client.Update(ctx, newSvc); err != nil {
			logger.Error(err, "failed to update service", "Name", req.NamespacedName)
			return ctrl.Result{}, err
		}
		return ctrl.Result{}, nil
	}
	if !reflect.DeepEqual(svc.Status, newSvc.Status) {
		// update status
		if err := r.Client.Status().Update(ctx, newSvc); err != nil {
			logger.Error(err, "failed to update service status", "Name", req.NamespacedName)
			return ctrl.Result{}, err
		}
		return ctrl.Result{}, nil
	}

	// nothing to update
	logger.Info("nothing to update service", "Name", req.NamespacedName)
	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) reconcileServiceWhenDelete(ctx context.Context, req ctrl.Request) (ctrl.Result, error) {
	logger := log.FromContext(ctx)

	logger.Info("reconcile when deleting", "Name", req.NamespacedName)

	return ctrl.Result{}, nil
}

func (r *LBAllocationReconciler) handleLocal(ctx context.Context, svc *v1.Service) error {
	logger := log.FromContext(ctx)
	nsName := types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}

	// list endpoint slices
	epSliceList := discoveryv1.EndpointSliceList{}
	if err := r.Client.List(ctx, &epSliceList, client.MatchingLabels{constants.KubernetesServiceNameLabel: svc.Name}); err != nil {
		logger.Error(err, "failed to list EndpointSlice by Service", "ServiceName", nsName)
		return err
	}

	// allocation
	newSvc := svc.DeepCopy()
	poolName, allocatedAddrs, err := r.allocate(ctx, newSvc, epSliceList)
	if err != nil {
		logger.Error(err, "failed to allocate a LB address", "Name", nsName)
		// clear allocation information from svc
		if err := r.release(logger, newSvc); err != nil {
			logger.Error(err, "failed to release", "Name", nsName)
			return err
		}
		return err
	}
	if err := r.setAllocationInfo(newSvc, poolName, allocatedAddrs); err != nil {
		logger.Error(err, "failed to set allocation information", "Name", nsName)
		return err
	}
	return nil
}

func (r *LBAllocationReconciler) handleCluster(ctx context.Context, svc *v1.Service) error {
	return nil
}

// handle BGPAdvertisement
func (r *LBAllocationReconciler) assign(ctx context.Context, req ctrl.Request) error {
	logger := log.FromContext(ctx)

	logger.Info("try to assign lb address", "Name", req.NamespacedName)
	adv := &sartv1alpha1.BGPAdvertisement{}
	if err := r.Client.Get(ctx, req.NamespacedName, adv); err != nil {
		if apierrors.IsNotFound(err) {
			return err
		}
		logger.Error(err, "failed to get BGPAdvertisement", "Name", req.NamespacedName)
		return err
	}
	if adv.Status != sartv1alpha1.BGPAdvertisementStatusAdvertised {
		return nil
	}

	// update
	svcNamespace, svcName := advertiseNameToServiceName(req.Name)
	svc := &v1.Service{}
	if err := r.Client.Get(ctx, types.NamespacedName{Namespace: svcNamespace, Name: svcName}, svc); err != nil {
		if apierrors.IsNotFound(err) {
			logger.Info("failed to find the Service corresponding BGPAdvertisements", "Name", types.NamespacedName{Namespace: svcNamespace, Name: svcName})
			return nil
		}
		logger.Error(err, "failed to get Service", "Name", req.NamespacedName)
		return err
	}
	// assign an allocated address to the load balancer service
	allocatedIpPrefix, err := netip.ParsePrefix(adv.Spec.Network)
	if err != nil {
		return err
	}
	addrMap := make(map[string]netip.Addr)
	for _, ingress := range svc.Status.LoadBalancer.Ingress {
		a, err := netip.ParseAddr(ingress.IP)
		if err != nil {
			return err
		}
		addrMap[protocolFromAddr(a)] = a
	}

	addrMap[protocolFromAddr(allocatedIpPrefix.Addr())] = allocatedIpPrefix.Addr()

	newSvc := svc.DeepCopy()

	lbAddrs := []v1.LoadBalancerIngress{}
	for _, a := range addrMap {
		lbAddrs = append(lbAddrs, v1.LoadBalancerIngress{IP: a.String()})
	}
	newSvc.Status.LoadBalancer.Ingress = lbAddrs

	newSvc.Status.Conditions = append(newSvc.Status.Conditions, metav1.Condition{
		Type:    ServiceConditionReasonAdvertised,
		Status:  metav1.ConditionTrue,
		Reason:  ServiceConditionReasonAdvertised,
		Message: "allocated addresses are advertised and assigned",
	})

	// update service status
	if reflect.DeepEqual(svc, newSvc) {
		return nil
	}

	logger.Info("assign lb address", "Name", types.NamespacedName{Namespace: svcNamespace, Name: svcName}, "Address", lbAddrs)
	if err := r.Client.Status().Update(ctx, newSvc); err != nil {
		logger.Error(err, "failed to update service status", "Name", types.NamespacedName{Namespace: svcNamespace, Name: svcName})
		return err
	}
	return nil
}

func (r *LBAllocationReconciler) allocate(ctx context.Context, svc *v1.Service, eps discoveryv1.EndpointSliceList) (string, []netip.Addr, error) {
	logger := log.FromContext(ctx)

	// check protocol by clusterIP
	if len(svc.Spec.ClusterIP) == 0 && svc.Spec.ClusterIP == "" {
		return "", nil, fmt.Errorf("Service doesn't have ClusterIP")
	}

	poolName, ok := svc.Annotations[constants.AnnotationAddressPool]
	if !ok {
		return "", nil, fmt.Errorf("required annotation %s is not found to address allocated load balancer", constants.AnnotationAddressPool)
	}

	lbIPs := []netip.Addr{}
	// check allocated LB IP
	for _, ingress := range svc.Status.LoadBalancer.Ingress {
		addr, err := netip.ParseAddr(ingress.IP)
		if err != nil {
			return "", nil, err
		}
		lbIPs = append(lbIPs, addr)
	}

	desiredIPs := make(map[string]netip.Addr)
	if len(lbIPs) > 0 {
		logger.Info("LB address is already allocated", "Name", svc.Name, "Addresses", lbIPs)
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
		for _, ip := range lbIPs {
			// specified lb address, but not allocated
			if ip.Is4() {
				if protocolAllocator[constants.ProtocolIpv4].IsAllocated(ip) {
					logger.Info("ipv4 address is allocated", "Pool", from, "Address", ip)
					break
				}
				desiredIPs[constants.ProtocolIpv4] = ip
			} else {
				logger.Info("ipv6 address is allocated", "Pool", from, "Address", ip)
				if protocolAllocator[constants.ProtocolIpv6].IsAllocated(ip) {
					break
				}
				desiredIPs[constants.ProtocolIpv6] = ip
			}
		}
		return poolName, lbIPs, nil
	}

	// get desired address from service spec
	// TODO: I'm not sure when we handle dual stack
	if svc.Spec.LoadBalancerIP != "" {
		logger.Info("Spec.LoadBalancer is specified", "LoadBalancerIP", svc.Spec.LoadBalancerIP)
		ipStrs := strings.Split(svc.Spec.LoadBalancerIP, ",")
		for _, is := range ipStrs {
			ip, err := netip.ParseAddr(is)
			if err != nil {
				return "", nil, err
			}
			if ip.Is4() {
				desiredIPs[constants.ProtocolIpv4] = ip
			} else {
				desiredIPs[constants.ProtocolIpv6] = ip
			}
		}
	}

	// check if already allocated
	needAllocation := false
	adv := &sartv1alpha1.BGPAdvertisement{}
	for protocol, _ := range r.Allocators[poolName] {
		advName := svcToAdvertisementName(svc, protocol)
		if err := r.Client.Get(ctx, types.NamespacedName{Namespace: constants.Namespace, Name: advName}, adv); err != nil {
			if apierrors.IsNotFound(err) {
				logger.Info("need to allocate lb addresses", "Name", types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}, "Pool", poolName)
				needAllocation = true
				break
			}
			return poolName, nil, err
		}
		logger.Info("already allocated", "Name", types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}, "Pool", poolName, "Address")
		// select backends
		newAdv, needReAdvertise, err := r.selectBackend(svc, eps, adv)
		if err != nil {
			logger.Error(err, "failed to select backends", "Name", types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name})
			return poolName, nil, err
		}
		if needReAdvertise {
			if err := r.Client.Update(ctx, newAdv); err != nil {
				logger.Error(err, "failed to udpate advertisement", "Name", types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}, "Advertisement", newAdv.Name)
				return poolName, nil, err
			}
		}
	}

	if !needAllocation {
		return poolName, nil, nil
	}
	// allocation
	logger.Info("allocate by each allocator", "Pool", poolName, "Desired", desiredIPs)
	allocated := make(map[string]netip.Addr)
	for protocol, allocator := range r.Allocators[poolName] {
		desired, ok := desiredIPs[protocol]
		if ok {
			addr, err := allocator.Allocate(desired)
			if err != nil {
				return "", nil, err
			}
			logger.Info("Allocate from pool", "Pool", poolName, "Protocol", protocol, "Address", addr)
			allocated[protocol] = addr
			logger.Info("used addresses", "Bits", allocator.Allocated())
		} else {
			addr, err := allocator.AllocateNext()
			if err != nil {
				return "", nil, err
			}
			logger.Info("Allocate next from pool", "Pool", poolName, "Protocol", protocol, "Address", addr)
			allocated[protocol] = addr
			logger.Info("used addresses", "Bits", allocator.Allocated())
		}
	}

	logger.Info("allocate lb address", "Pool", poolName, "Addresses", allocated)
	a := make([]netip.Addr, 0, 2)
	for _, v := range allocated {
		a = append(a, v)
	}
	return poolName, a, nil
}

func (r *LBAllocationReconciler) release(logger logr.Logger, svc *v1.Service) error {
	delete(svc.Annotations, constants.AnnotationAllocatedFromPool)
	if svc.Status.LoadBalancer.Ingress == nil || len(svc.Status.LoadBalancer.Ingress) == 0 {
		return nil
	}
	poolName, ok := svc.Annotations[constants.AnnotationAddressPool]
	if !ok {
		return fmt.Errorf("pool is not found")
	}
	allocator, ok := r.Allocators[poolName]
	if !ok {
		return fmt.Errorf("allocator is not registered")
	}

	for _, ingress := range svc.Status.LoadBalancer.Ingress {
		addr, err := netip.ParseAddr(ingress.IP)
		if err != nil {
			return err
		}
		if addr.Is4() {
			_, err := allocator[constants.ProtocolIpv4].Release(addr)
			if err != nil {
				return err
			}
		} else {
			_, err := allocator[constants.ProtocolIpv6].Release(addr)
			if err != nil {
				return err
			}
		}
	}
	svc.Status.LoadBalancer = v1.LoadBalancerStatus{}
	return nil
}

func (r *LBAllocationReconciler) setAllocationInfo(svc *v1.Service, poolName string, addrs []netip.Addr) error {
	p, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
	if !ok || p != poolName {
		svc.Annotations[constants.AnnotationAllocatedFromPool] = poolName
	}
	addrMap := make(map[string]netip.Addr)
	for _, lbAddr := range svc.Status.LoadBalancer.Ingress {
		a, err := netip.ParseAddr(lbAddr.IP)
		if err != nil {
			return err
		}
		if a.Is4() {
			addrMap[constants.ProtocolIpv4] = a
		} else {
			addrMap[constants.ProtocolIpv6] = a
		}
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
			return poolName, nil, fmt.Errorf("allocated addresses and stored addresses is not matched")
		}
	}

	return poolName, lbIPs, nil
}

func (r *LBAllocationReconciler) isAllocated(svc *v1.Service) (bool, error) {
	poolName, ok := svc.Annotations[constants.AnnotationAddressPool]
	if !ok {
		return false, ErrPoolAnnotationIsNotFound
	}
	from, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
	if !ok {
		// not allocated yet
		return false, nil
	}
	if poolName != from {
		return false, fmt.Errorf("pool name is not matched")
	}

	allocs, ok := r.allocMap[types.NamespacedName{Namespace: svc.Namespace, Name: svc.Name}.String()]
	if !ok {
		return false, nil
	}

	for _, alloc := range allocs {
		allocators, ok := r.Allocators[poolName]
		if !ok {
			return false, ErrAllocatorIsNotFound
		}
		allocator, ok := allocators[alloc.protocol]
		if !ok {
			return false, ErrAllocatorIsNotFound
		}
		if !allocator.IsAllocated(alloc.addr) {
			return false, fmt.Errorf("expected allocation is not satisfied %s", alloc)
		}
	}

	return true, nil
}

func desiredAddr(svc *v1.Service) (map[string]netip.Addr, error) {
	desired := make(map[string]netip.Addr)
	if svc.Spec.LoadBalancerIP == "" {
		// no desired addresses
		return nil, nil
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

func (r *LBAllocationReconciler) clearAllocationInfo(svc *v1.Service) error {
	delete(svc.Annotations, constants.AnnotationAllocatedFromPool)
	svc.Status.LoadBalancer = v1.LoadBalancerStatus{}
	return nil
}

func (r *LBAllocationReconciler) createOrUpdateAdvertisement(ctx context.Context, svcName string, lbAddr netip.Addr, epSliceList discoveryv1.EndpointSliceList) (*sartv1alpha1.BGPAdvertisement, bool, error) {
	logger := log.FromContext(ctx)

	prefix := netip.PrefixFrom(lbAddr, 32)
	nodes := []string{}
	for _, eps := range epSliceList.Items {
		for _, ep := range eps.Endpoints {
			if ep.NodeName == nil || *ep.NodeName == "" {
				continue
			}
			nodes = append(nodes, *ep.NodeName)
		}
	}

	needUpdate := false

	advertisement := &sartv1alpha1.BGPAdvertisement{}
	if err := r.Client.Get(ctx, types.NamespacedName{Name: svcName, Namespace: constants.Namespace}, advertisement); err != nil {
		if apierrors.IsNotFound(err) {
			// advertise is not exist
			// create it
			needUpdate = true

			advertisement.Name = svcName
			advertisement.Namespace = constants.Namespace

			logger.Info("create new advertisement", "Name", svcName, "Prefix", prefix, "Nodes", nodes)
			spec := sartv1alpha1.BGPAdvertisementSpec{
				Network:   prefix.String(),
				Type:      AdvertisementTypeService,
				Protocol:  protocolFromAddr(lbAddr),
				Origin:    "",
				LocalPref: 0,
				Nodes:     nodes,
			}
			advertisement.Spec = spec
			return advertisement, false, nil
		} else {
			logger.Error(err, "failed to get BgpAdvertisement", "Name", svcName)
			return nil, false, err
		}
	}

	if prefix.String() != advertisement.Spec.Network {
		advertisement.Spec.Network = prefix.String()
		needUpdate = true
	}
	if !reflect.DeepEqual(advertisement.Spec.Nodes, nodes) {
		advertisement.Spec.Nodes = nodes
		needUpdate = true
	}

	if needUpdate {
		return advertisement, true, nil
	}

	return nil, false, nil
}

func (r *LBAllocationReconciler) selectBackend(svc *v1.Service, epsList discoveryv1.EndpointSliceList, adv *sartv1alpha1.BGPAdvertisement) (*sartv1alpha1.BGPAdvertisement, bool, error) {

	_, ok := svc.Annotations[constants.AnnotationAllocatedFromPool]
	if !ok {
		return nil, false, fmt.Errorf("service not found")
	}
	nodes := []string{}
	for _, eps := range epsList.Items {
		for _, ep := range eps.Endpoints {
			if ep.NodeName == nil || *ep.NodeName == "" {
				continue
			}
			nodes = append(nodes, *ep.NodeName)
		}
	}
	newAdv := adv.DeepCopy()
	sort.Strings(newAdv.Spec.Nodes)
	sort.Strings(nodes)

	if !reflect.DeepEqual(newAdv.Spec.Nodes, nodes) {
		newAdv.Spec.Nodes = nodes
		newAdv.Status = sartv1alpha1.BGPAdvertisementStatusUpdated
		return newAdv, true, nil
	}
	return nil, false, nil
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
	return svc.Namespace + "-" + svc.Name + "-" + protocol
}

func advertiseNameToServiceName(adv string) (string, string) {
	s := strings.SplitN(adv, "-", 2)
	if len(s) < 2 {
		return "", ""
	}
	namespace := s[0]

	name := strings.TrimSuffix(s[1], "-"+constants.ProtocolIpv4)
	name = strings.TrimSuffix(name, "-"+constants.ProtocolIpv6)

	return namespace, name
}
