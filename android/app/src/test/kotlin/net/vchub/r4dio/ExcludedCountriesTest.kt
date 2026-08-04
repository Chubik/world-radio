package net.vchub.r4dio

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ExcludedCountriesTest {
    @Test
    fun changed_isFalse_forIdenticalSets() {
        assertFalse(ExcludedCountries.changed(setOf("US", "GB"), setOf("US", "GB")))
    }

    @Test
    fun changed_isFalse_forSameSetDifferentOrder() {
        assertFalse(ExcludedCountries.changed(setOf("US", "GB", "FR"), setOf("FR", "US", "GB")))
    }

    @Test
    fun changed_isFalse_forCaseOnlyDifference() {
        assertFalse(ExcludedCountries.changed(setOf("us", "GB"), setOf("US", "gb")))
    }

    @Test
    fun changed_isTrue_whenACountryIsAdded() {
        assertTrue(ExcludedCountries.changed(setOf("US"), setOf("US", "GB")))
    }

    @Test
    fun changed_isTrue_whenACountryIsRemoved() {
        assertTrue(ExcludedCountries.changed(setOf("US", "GB"), setOf("US")))
    }

    @Test
    fun changed_isTrue_fromEmptyToNonEmpty() {
        assertTrue(ExcludedCountries.changed(emptySet(), setOf("US")))
    }

    @Test
    fun changed_isFalse_forEmptyToEmpty() {
        assertFalse(ExcludedCountries.changed(emptySet(), emptySet()))
    }

    @Test
    fun normalize_upperCasesEveryEntry() {
        assertTrue(ExcludedCountries.normalize(setOf("us", "gb")) == setOf("US", "GB"))
    }
}
