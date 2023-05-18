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

package v1alpha1

import (
	"errors"

	"k8s.io/apimachinery/pkg/runtime"
	ctrl "sigs.k8s.io/controller-runtime"
	logf "sigs.k8s.io/controller-runtime/pkg/log"
	"sigs.k8s.io/controller-runtime/pkg/webhook"
)

// log is for logging in this package.
var bgpadvertisementlog = logf.Log.WithName("bgpadvertisement-resource")

var OwnerReferenceByServiceNotFound error = errors.New("Owner reference by Service is not found")

func (r *BGPAdvertisement) SetupWebhookWithManager(mgr ctrl.Manager) error {
	return ctrl.NewWebhookManagedBy(mgr).
		For(r).
		Complete()
}

// TODO(user): EDIT THIS FILE!  THIS IS SCAFFOLDING FOR YOU TO OWN!

// TODO(user): change verbs to "verbs=create;update;delete" if you want to enable deletion validation.
//+kubebuilder:webhook:path=/validate-sart-terassyi-net-v1alpha1-bgpadvertisement,mutating=false,failurePolicy=fail,sideEffects=None,groups=sart.terassyi.net,resources=bgpadvertisements,verbs=create;update,versions=v1alpha1,name=vbgpadvertisement.kb.io,admissionReviewVersions=v1

var _ webhook.Validator = &BGPAdvertisement{}

// ValidateCreate implements webhook.Validator so a webhook will be registered for the type
func (r *BGPAdvertisement) ValidateCreate() error {
	bgpadvertisementlog.Info("validate create", "name", r.Name)

	if hasOwnerReferenceByService(r) {
		return nil
	}
	return NodeBGPOwnerReferenceNotFound
}

// ValidateUpdate implements webhook.Validator so a webhook will be registered for the type
func (r *BGPAdvertisement) ValidateUpdate(old runtime.Object) error {
	bgpadvertisementlog.Info("validate update", "name", r.Name)

	if hasOwnerReferenceByService(r) {
		return nil
	}
	return NodeBGPOwnerReferenceNotFound
}

// ValidateDelete implements webhook.Validator so a webhook will be registered for the type
func (r *BGPAdvertisement) ValidateDelete() error {
	bgpadvertisementlog.Info("validate delete", "name", r.Name)

	// TODO(user): fill in your validation logic upon object deletion.
	return nil
}

func hasOwnerReferenceByService(adv *BGPAdvertisement) bool {
	for _, ref := range adv.OwnerReferences {
		if ref.Kind == "Service" {
			return true
		}
	}
	return false
}