package com.tophunt.forge.data

import android.content.Context
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow

/** Whether the device has a usable network. Screens show an offline state from it. */
interface Connectivity {
    val online: StateFlow<Boolean>
}

class FixedConnectivity(online: Boolean = true) : Connectivity {
    override val online = MutableStateFlow(online)
}

class SystemConnectivity(context: Context) : Connectivity {
    private val cm = context.getSystemService(ConnectivityManager::class.java)
    private val state = MutableStateFlow(current())
    override val online: StateFlow<Boolean> = state

    init {
        cm?.registerDefaultNetworkCallback(object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) { state.value = true }
            override fun onLost(network: Network) { state.value = current() }
            override fun onCapabilitiesChanged(network: Network, caps: NetworkCapabilities) {
                state.value = caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
            }
        })
    }

    private fun current(): Boolean {
        val caps = cm?.activeNetwork?.let { cm.getNetworkCapabilities(it) } ?: return false
        return caps.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
    }
}
