package expo.modules.vautr

import android.content.Context
import expo.modules.core.BasePackage
import expo.modules.core.interfaces.InternalModuleProvider
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

/**
 * Registers [VautrNativeModule] with the Expo runtime (VTR-061).
 */
class VautrNativePackage : BasePackage() {
    override fun createExportedModules(context: Context): List<Module> {
        return listOf(VautrNativeModule())
    }
}
