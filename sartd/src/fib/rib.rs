use ipnet::IpNet;

use std::collections::HashMap;

use super::route::Route;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RequestType {
    AddRoute,
    DeleteRoute,
    AddMultiPathRoute,
    DeleteMultiPathRoute,
}

#[derive(Debug)]
pub(crate) struct Rib {
    pub ip_version: rtnetlink::IpVersion,
    pub table: Table,
}

impl Default for Rib {
    fn default() -> Self {
        Self {
            ip_version: rtnetlink::IpVersion::V4,
            table: Table {
                inner: HashMap::new(),
            },
        }
    }
}

#[derive(Debug)]
pub(crate) struct Table {
    inner: HashMap<IpNet, Vec<Route>>,
}

impl Rib {
    pub fn new(protocol: rtnetlink::IpVersion) -> Self {
        Self {
            ip_version: protocol,
            table: Table {
                inner: HashMap::new(),
            },
        }
    }

    pub fn insert(&mut self, destination: IpNet, route: Route) -> Option<Route> {
        if route.version != self.ip_version {
            return None;
        }
        match self.table.inner.get_mut(&destination) {
            Some(routes) => {
                if routes.is_empty() {
                    routes.push(route.clone());
                    return Some(route);
                }
                let old_published = routes[0].clone();
                if let Some(existing) = routes.iter_mut().find(|r| r.protocol == route.protocol) {
                    *existing = route;
                } else {
                    routes.push(route);
                }
                routes.sort();
                let new_published = routes[0].clone();
                if new_published > old_published {
                    return None;
                }
                Some(new_published)
            }
            None => {
                let routes = vec![route.clone()];
                self.table.inner.insert(destination, routes);
                Some(route)
            }
        }
    }

    pub fn get(&self, destination: &IpNet) -> Option<&Vec<Route>> {
        self.table.inner.get(destination)
    }

    pub fn list(&self) -> Option<Vec<Route>> {
        let mut all_routes = Vec::new();
        for (_, routes) in self.table.inner.iter() {
            all_routes.push(routes.clone());
        }
        if all_routes.is_empty() {
            return None;
        }
        Some(all_routes.into_iter().flatten().collect())
    }

    pub fn get_published(&self, destination: &IpNet) -> Option<&Route> {
        let routes = self.table.inner.get(destination)?;
        if routes.is_empty() {
            return None;
        }
        Some(&routes[0])
    }

    pub fn remove(&mut self, route: Route) -> Option<Route> {
        if route.version != self.ip_version {
            return None;
        }
        match self.table.inner.get_mut(&route.destination) {
            Some(routes) => {
                if routes.is_empty() {
                    return None;
                }
                let index = match routes.iter().position(|r| {
                    if r.next_hops.len() != route.next_hops.len() {
                        return false;
                    }
                    for i in 0..r.next_hops.len() - 1 {
                        if !r.next_hops[i].gateway.eq(&route.next_hops[i].gateway) {
                            return false;
                        }
                    }
                    true
                }) {
                    Some(index) => index,
                    None => return None,
                };
                let ret_route = routes.remove(index);
                if index == 0 {
                    // removed a published route
                    return Some(ret_route);
                }
                None
            }
            None => None,
        }
    }
}

#[cfg(test)]
mod tests {}
