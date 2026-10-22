package com.spaceport.client

import android.app.Activity
import android.content.pm.ActivityInfo
import android.graphics.Color
import android.net.Uri
import android.view.ViewGroup
import android.webkit.WebView
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.annotation.OptIn
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.TrackSelectionOverride
import androidx.media3.common.Tracks
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DefaultDataSource
import androidx.media3.datasource.DefaultHttpDataSource
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.source.DefaultMediaSourceFactory
import androidx.media3.ui.CaptionStyleCompat
import androidx.media3.ui.PlayerView
import androidx.media3.ui.SubtitleView
import app.tauri.plugin.JSObject
import org.json.JSONArray

/**
 * The Android video player: Media3 (ExoPlayer) in a view under the web page, which draws the
 * controls over it. Rust drives it through SpaceportPlugin (src-tauri/src/player/exo.rs) and
 * reads its state about four times a second with poll(). Every method runs on the UI thread.
 */
@OptIn(UnstableApi::class)
class SpaceportPlayer(private val activity: Activity) {
  private var player: ExoPlayer? = null
  private var view: PlayerView? = null
  private var webView: WebView? = null

  /** What happened since the last poll, for Rust: loaded, ended, back, error:<text>. */
  private val events = mutableListOf<String>()
  /** A new stream was asked for; "loaded" is sent once it is ready. */
  private var loadPending = false
  private var url: String? = null
  /** Subtitle files added from addons; Media3 takes them as part of the media item. */
  private val subtitles = mutableListOf<MediaItem.SubtitleConfiguration>()
  /** The label of an added subtitle to show once its track appears. */
  private var subtitleToSelect: String? = null
  private var volume = 1f
  private var muted = false
  private val style = mutableMapOf<String, String>()

  private val back = object : OnBackPressedCallback(false) {
    override fun handleOnBackPressed() {
      events.add("back")
    }
  }

  private val listener = object : Player.Listener {
    override fun onPlaybackStateChanged(state: Int) {
      if (state == Player.STATE_READY && loadPending) {
        loadPending = false
        events.add("loaded")
      }
      if (state == Player.STATE_ENDED) events.add("ended")
    }

    override fun onPlayerError(error: PlaybackException) {
      loadPending = false
      val reason = error.errorCodeName.removePrefix("ERROR_CODE_").lowercase().replace('_', ' ')
      events.add("error:$reason")
    }

    override fun onTracksChanged(tracks: Tracks) {
      val label = subtitleToSelect ?: return
      val index = trackList(C.TRACK_TYPE_TEXT).indexOfFirst { (group, i) ->
        group.getTrackFormat(i).label == label
      }
      if (index >= 0) {
        subtitleToSelect = null
        select(C.TRACK_TYPE_TEXT, index + 1)
      }
    }
  }

  /** Called once the web view exists: the back button closes the player while one is open. */
  fun attach(webView: WebView) {
    this.webView = webView
    (activity as? ComponentActivity)?.onBackPressedDispatcher?.addCallback(back)
  }

  /** Shows the video view behind the page, in landscape without the system bars. */
  fun open() {
    if (player != null) return
    val http = DefaultHttpDataSource.Factory()
      .setUserAgent(USER_AGENT)
      .setAllowCrossProtocolRedirects(true)
      // Torrents may need this long to fetch their file list before the first byte arrives.
      .setConnectTimeoutMs(TIMEOUT_MS)
      .setReadTimeoutMs(TIMEOUT_MS)
    val exo = ExoPlayer.Builder(activity)
      .setMediaSourceFactory(DefaultMediaSourceFactory(DefaultDataSource.Factory(activity, http)))
      .setAudioAttributes(
        AudioAttributes.Builder()
          .setUsage(C.USAGE_MEDIA)
          .setContentType(C.AUDIO_CONTENT_TYPE_MOVIE)
          .build(),
        true,
      )
      .setHandleAudioBecomingNoisy(true)
      .build()
    exo.addListener(listener)
    exo.playWhenReady = true
    val playerView = PlayerView(activity).apply {
      useController = false
      player = exo
      keepScreenOn = true
      setShutterBackgroundColor(Color.BLACK)
      setBackgroundColor(Color.BLACK)
    }
    val root = activity.findViewById<ViewGroup>(android.R.id.content)
    root.addView(
      playerView,
      0,
      ViewGroup.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT),
    )
    webView?.setBackgroundColor(Color.TRANSPARENT)
    player = exo
    view = playerView
    applyStyle()
    activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_SENSOR_LANDSCAPE
    WindowCompat.getInsetsController(activity.window, activity.window.decorView).apply {
      systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
      hide(WindowInsetsCompat.Type.systemBars())
    }
    back.isEnabled = true
  }

  fun close() {
    player?.release()
    player = null
    view?.let { (it.parent as? ViewGroup)?.removeView(it) }
    view = null
    events.clear()
    subtitles.clear()
    subtitleToSelect = null
    loadPending = false
    url = null
    back.isEnabled = false
    activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_UNSPECIFIED
    WindowCompat.getInsetsController(activity.window, activity.window.decorView)
      .show(WindowInsetsCompat.Type.systemBars())
  }

  /** Stops playing when the app goes to the background. */
  fun pause() {
    player?.playWhenReady = false
  }

  /** Carries out one operation from Rust (`Op` in exo.rs). */
  fun op(args: JSObject) {
    val exo = player ?: return
    when (args.getString("op")) {
      "load" -> {
        url = args.getString("url")
        subtitles.clear()
        subtitleToSelect = null
        loadPending = true
        val start = if (args.isNull("start")) 0L else (args.getDouble("start") * 1000).toLong()
        exo.setMediaItem(mediaItem(), start)
        exo.prepare()
        exo.playWhenReady = true
      }
      "togglePause" -> exo.playWhenReady = !exo.playWhenReady
      "seek" -> {
        val ms = (args.getDouble("seconds") * 1000).toLong()
        val target = if (args.getBoolean("relative")) exo.currentPosition + ms else ms
        exo.seekTo(target.coerceAtLeast(0))
      }
      "toggleMute" -> {
        muted = !muted
        exo.volume = if (muted) 0f else volume
      }
      "volume" -> {
        volume = (args.getDouble("value") / 100).toFloat().coerceIn(0f, 1f)
        muted = false
        exo.volume = volume
      }
      "speed" -> exo.setPlaybackSpeed(args.getDouble("value").toFloat())
      "audioTrack" -> select(C.TRACK_TYPE_AUDIO, args.getInt("id"))
      "subtitleTrack" ->
        if (args.isNull("id")) {
          exo.trackSelectionParameters = exo.trackSelectionParameters.buildUpon()
            .setTrackTypeDisabled(C.TRACK_TYPE_TEXT, true)
            .build()
        } else {
          select(C.TRACK_TYPE_TEXT, args.getInt("id"))
        }
      "addSubtitle" -> addSubtitle(args.getString("url")!!, args.getString("title")!!, args.getString("lang")!!)
      "audioLanguages" ->
        exo.trackSelectionParameters = exo.trackSelectionParameters.buildUpon()
          .setPreferredAudioLanguages(*strings(args.getJSONArray("languages")))
          .build()
      "subtitleLanguages" ->
        exo.trackSelectionParameters = exo.trackSelectionParameters.buildUpon()
          .setPreferredTextLanguages(*strings(args.getJSONArray("languages")))
          .build()
      "style" -> {
        style[args.getString("name")!!] = args.getString("value")!!
        applyStyle()
      }
    }
  }

  /** The state Rust turns into mpv-style events (`Poll` in exo.rs). */
  fun poll(): JSObject {
    val result = JSObject()
    val happened = JSONArray()
    events.forEach { happened.put(it) }
    events.clear()
    result.put("events", happened)
    val exo = player ?: return result
    val duration = exo.duration
    result.put("position", exo.currentPosition / 1000.0)
    result.put("duration", if (duration == C.TIME_UNSET) 0.0 else duration / 1000.0)
    result.put("paused", !exo.playWhenReady)
    result.put("buffering", exo.playbackState == Player.STATE_BUFFERING)
    result.put("volume", volume * 100.0)
    result.put("muted", muted)
    result.put("speed", exo.playbackParameters.speed.toDouble())
    result.put("seekable", exo.isCurrentMediaItemSeekable)
    val tracks = JSONArray()
    trackList(C.TRACK_TYPE_AUDIO).forEachIndexed { n, (group, i) ->
      tracks.put(describe(n + 1, "audio", group, i))
    }
    trackList(C.TRACK_TYPE_TEXT).forEachIndexed { n, (group, i) ->
      tracks.put(describe(n + 1, "sub", group, i))
    }
    result.put("tracks", tracks)
    return result
  }

  private fun mediaItem(): MediaItem =
    MediaItem.Builder().setUri(url).setSubtitleConfigurations(subtitles.toList()).build()

  /**
   * Media3 only takes subtitle files with the media item, so the item is set again with the new
   * file and playback carries on from where it was.
   */
  private fun addSubtitle(link: String, title: String, lang: String) {
    val exo = player ?: return
    if (subtitles.none { it.uri.toString() == link }) {
      subtitles.add(
        MediaItem.SubtitleConfiguration.Builder(Uri.parse(link))
          .setMimeType(subtitleType(link))
          .setLabel(title)
          .setLanguage(lang.ifEmpty { null })
          .build()
      )
      val position = exo.currentPosition
      exo.setMediaItem(mediaItem(), position)
      exo.prepare()
    }
    subtitleToSelect = subtitles.first { it.uri.toString() == link }.label
    listener.onTracksChanged(exo.currentTracks)
  }

  /** The playable tracks of one type, in the order their ids (from 1) refer to. */
  private fun trackList(type: Int): List<Pair<Tracks.Group, Int>> {
    val exo = player ?: return emptyList()
    return exo.currentTracks.groups
      .filter { it.type == type }
      .flatMap { group -> (0 until group.length).filter { group.isTrackSupported(it) }.map { group to it } }
  }

  private fun select(type: Int, id: Int) {
    val exo = player ?: return
    val (group, index) = trackList(type).getOrNull(id - 1) ?: return
    exo.trackSelectionParameters = exo.trackSelectionParameters.buildUpon()
      .setTrackTypeDisabled(type, false)
      .setOverrideForType(TrackSelectionOverride(group.mediaTrackGroup, index))
      .build()
  }

  /** One track in the shape of an entry of mpv's track-list. */
  private fun describe(id: Int, kind: String, group: Tracks.Group, index: Int): JSObject {
    val format = group.getTrackFormat(index)
    val track = JSObject()
    track.put("id", id)
    track.put("type", kind)
    track.put("selected", group.isTrackSelected(index))
    format.label?.let { track.put("title", it) }
    format.language?.takeIf { it != C.LANGUAGE_UNDETERMINED }?.let { track.put("lang", it) }
    codecName(format)?.let { track.put("codec", it) }
    if (format.channelCount != Format.NO_VALUE) track.put("demux-channel-count", format.channelCount)
    if (format.selectionFlags and C.SELECTION_FLAG_FORCED != 0) track.put("forced", true)
    if (kind == "sub") {
      subtitles.firstOrNull { it.label != null && it.label == format.label }
        ?.let { track.put("external-filename", it.uri.toString()) }
    }
    return track
  }

  /** The subtitle size, colour, background and height from Settings (mpv's names). */
  private fun applyStyle() {
    val subtitleView = view?.subtitleView ?: return
    val color = parseColor(style["sub-color"], Color.WHITE)
    val box = style["sub-border-style"] == "background-box"
    subtitleView.setStyle(
      if (box) {
        CaptionStyleCompat(
          color,
          parseColor(style["sub-back-color"], 0xB0000000.toInt()),
          Color.TRANSPARENT,
          CaptionStyleCompat.EDGE_TYPE_NONE,
          Color.BLACK,
          null,
        )
      } else {
        CaptionStyleCompat(
          color,
          Color.TRANSPARENT,
          Color.TRANSPARENT,
          CaptionStyleCompat.EDGE_TYPE_OUTLINE,
          Color.BLACK,
          null,
        )
      }
    )
    subtitleView.setApplyEmbeddedFontSizes(false)
    val scale = style["sub-scale"]?.toFloatOrNull() ?: 1f
    subtitleView.setFractionalTextSize(SubtitleView.DEFAULT_TEXT_SIZE_FRACTION * scale)
    // mpv's sub-pos: 100 is the bottom edge, smaller moves the subtitles up.
    val position = (style["sub-pos"]?.toFloatOrNull() ?: 100f).coerceIn(0f, 100f)
    subtitleView.setBottomPaddingFraction(SubtitleView.DEFAULT_BOTTOM_PADDING_FRACTION + (100f - position) / 100f)
  }

  private companion object {
    const val USER_AGENT = "Spaceport"
    const val TIMEOUT_MS = 30_000

    fun strings(array: JSONArray): Array<String> = Array(array.length()) { array.getString(it) }

    fun parseColor(text: String?, fallback: Int): Int =
      try {
        if (text == null) fallback else Color.parseColor(text)
      } catch (e: IllegalArgumentException) {
        fallback
      }

    fun subtitleType(link: String): String {
      val path = Uri.parse(link).lastPathSegment?.lowercase() ?: ""
      return when {
        path.endsWith(".vtt") -> MimeTypes.TEXT_VTT
        path.endsWith(".ass") || path.endsWith(".ssa") -> MimeTypes.TEXT_SSA
        path.endsWith(".ttml") || path.endsWith(".dfxp") -> MimeTypes.APPLICATION_TTML
        else -> MimeTypes.APPLICATION_SUBRIP
      }
    }

    /** A short codec name like mpv's ("eac3", "subrip"), from the track's MIME type. */
    fun codecName(format: Format): String? {
      // Subtitles parsed while the file is read keep their original type in `codecs`.
      val mime = (
        if (format.sampleMimeType == MimeTypes.APPLICATION_MEDIA3_CUES) format.codecs
        else format.sampleMimeType
      ) ?: return null
      return when (mime) {
        MimeTypes.AUDIO_AAC -> "aac"
        MimeTypes.AUDIO_AC3 -> "ac3"
        MimeTypes.AUDIO_E_AC3, MimeTypes.AUDIO_E_AC3_JOC -> "eac3"
        MimeTypes.AUDIO_TRUEHD -> "truehd"
        MimeTypes.AUDIO_DTS, MimeTypes.AUDIO_DTS_HD -> "dts"
        MimeTypes.APPLICATION_SUBRIP -> "subrip"
        MimeTypes.TEXT_SSA -> "ass"
        MimeTypes.TEXT_VTT -> "webvtt"
        MimeTypes.APPLICATION_PGS -> "pgs"
        else -> mime.substringAfter('/').removePrefix("x-")
      }
    }
  }
}
