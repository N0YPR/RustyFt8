#!/usr/bin/env python3
"""
Analyze decoder performance by adding timing instrumentation
"""

import re
import sys

def analyze_log():
    """Parse decoder timing logs"""
    
    with open('decoded.txt', 'r') as f:
        content = f.read()
    
    # Extract timing if available
    print("=== Decode Log Analysis ===")
    print(content)

if __name__ == '__main__':
    analyze_log()
