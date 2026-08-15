"""Slug generation for TextKit."""


def slugify(text):
    """Return a URL-friendly slug for ``text``.

    Behavior: lowercase the input, keep runs of ASCII-style alphanumerics,
    collapse every other character into a single hyphen separator, and never
    emit a leading or trailing hyphen. Input with no alphanumerics yields an
    empty string.
    """
    lowered = ""
    for character in text:
        lowered = lowered + character.lower()

    cleaned = ""
    for character in lowered:
        if character.isalnum():
            cleaned = cleaned + character
        else:
            cleaned = cleaned + " "

    parts = []
    current = ""
    for character in cleaned:
        if character == " ":
            if current != "":
                parts.append(current)
                current = ""
        else:
            current = current + character
    if current != "":
        parts.append(current)

    joined = ""
    for index, part in enumerate(parts):
        if index > 0:
            joined = joined + "-"
        joined = joined + part
    return joined
