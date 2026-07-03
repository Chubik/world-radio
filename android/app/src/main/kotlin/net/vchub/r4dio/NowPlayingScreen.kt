package net.vchub.r4dio

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Icon
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.foundation.clickable

private val BG = Color(0xFF15100B)
private val PANEL = Color(0xFF1B1510)
private val HI = Color(0xFFFFC457)
private val ACCENT = Color(0xFFFF8A3D)
private val DIM = Color(0xFF6E5430)
private val BRIGHT = Color(0xFFFFF0C0)
private val RULE = Color(0xFF3A2C17)

data class NowPlayingState(
    val station: String,
    val subtitle: String,
    val meta: String,
    val isPlaying: Boolean,
    val isFav: Boolean,
    val scope: Scope,
    val hint: String?,
)

@Composable
fun NowPlayingScreen(
    state: NowPlayingState,
    onShuffle: () -> Unit,
    onPlayPause: () -> Unit,
    onStar: () -> Unit,
    onScope: (Scope) -> Unit,
) {
    Column(
        modifier = Modifier.fillMaxSize().background(BG).padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text("r4dio", color = DIM, fontSize = 13.sp, modifier = Modifier.padding(top = 8.dp))
        Spacer(Modifier.weight(1f))
        Box(
            modifier = Modifier.size(160.dp).clip(RoundedCornerShape(16.dp)).background(PANEL),
            contentAlignment = Alignment.Center,
        ) {
            Icon(
                painter = painterResource(R.drawable.ic_stat_r4dio),
                contentDescription = null,
                tint = HI,
                modifier = Modifier.size(72.dp),
            )
        }
        Spacer(Modifier.height(18.dp))
        Text(state.station, color = BRIGHT, fontSize = 20.sp, fontWeight = FontWeight.SemiBold, textAlign = TextAlign.Center)
        if (state.subtitle.isNotBlank()) {
            Text(state.subtitle, color = ACCENT, fontSize = 13.sp, modifier = Modifier.padding(top = 4.dp), textAlign = TextAlign.Center)
        }
        if (state.meta.isNotBlank()) {
            Text(state.meta, color = DIM, fontSize = 12.sp, modifier = Modifier.padding(top = 4.dp))
        }
        Spacer(Modifier.weight(1f))
        Row(
            modifier = Modifier.clip(RoundedCornerShape(8.dp)).background(PANEL).padding(3.dp),
            horizontalArrangement = Arrangement.spacedBy(2.dp),
        ) {
            ScopeChip("ALL", state.scope == Scope.ALL) { onScope(Scope.ALL) }
            ScopeChip("★ FAVS", state.scope == Scope.FAVS) { onScope(Scope.FAVS) }
        }
        Spacer(Modifier.height(14.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Box(
                modifier = Modifier.weight(1f).height(52.dp).clip(RoundedCornerShape(12.dp)).background(HI).clickable { onShuffle() },
                contentAlignment = Alignment.Center,
            ) {
                Text("⇄ SHUFFLE", color = BG, fontWeight = FontWeight.Bold, fontSize = 15.sp)
            }
            CircleButton(if (state.isPlaying) R.drawable.ic_pause else R.drawable.ic_play, onPlayPause)
            StarButton(state.isFav, onStar)
        }
        state.hint?.let {
            Text(it, color = DIM, fontSize = 12.sp, modifier = Modifier.padding(top = 12.dp))
        }
        Spacer(Modifier.height(12.dp))
    }
}

@Composable
private fun ScopeChip(label: String, active: Boolean, onClick: () -> Unit) {
    Box(
        modifier = Modifier.clip(RoundedCornerShape(5.dp)).background(if (active) HI else Color.Transparent).clickable { onClick() }.padding(horizontal = 14.dp, vertical = 6.dp),
    ) {
        Text(label, color = if (active) BG else DIM, fontSize = 11.sp, fontWeight = if (active) FontWeight.Bold else FontWeight.Normal)
    }
}

@Composable
private fun CircleButton(icon: Int, onClick: () -> Unit) {
    Box(
        modifier = Modifier.size(52.dp).clip(CircleShape).background(PANEL).clickable { onClick() },
        contentAlignment = Alignment.Center,
    ) {
        Icon(painterResource(icon), contentDescription = null, tint = BRIGHT, modifier = Modifier.size(22.dp))
    }
}

@Composable
private fun StarButton(active: Boolean, onClick: () -> Unit) {
    Box(
        modifier = Modifier.size(52.dp).clip(CircleShape).background(PANEL).clickable { onClick() },
        contentAlignment = Alignment.Center,
    ) {
        Icon(painterResource(R.drawable.ic_star), contentDescription = null, tint = if (active) HI else DIM, modifier = Modifier.size(22.dp))
    }
}
